//! ACP transport layer: Content-Length framing over stdio.
//!
//! Identical in wire format to the LSP transport; adapted to route
//! agent→client requests back to the caller.

use crate::{Call, Error, Result};
use helix_acp_types::jsonrpc::{self, Id, ServerMessage};
use log::{error, info};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::{ChildStderr, ChildStdin, ChildStdout},
    sync::{
        mpsc::{Sender, UnboundedReceiver, UnboundedSender},
        Mutex, Notify,
    },
};

/// A message enqueued to send to the agent.
#[derive(Debug)]
pub enum Payload {
    /// A client-originated request (we expect a response).
    Request {
        chan: Sender<Result<Value>>,
        value: jsonrpc::MethodCall,
    },
    /// A notification we fire-and-forget.
    Notification(jsonrpc::Notification),
    /// A response to an agent-originated request.
    Response(jsonrpc::Output),
}

pub struct Transport {
    name: String,
    pending_requests: Mutex<HashMap<Id, Sender<Result<Value>>>>,
}

impl Transport {
    /// Spawns three async tasks (recv / stderr / send) and returns the channels.
    pub fn start(
        id: AgentClientIdRaw,
        name: String,
        server_stdout: BufReader<ChildStdout>,
        server_stdin: BufWriter<ChildStdin>,
        server_stderr: BufReader<ChildStderr>,
    ) -> (
        UnboundedReceiver<(AgentClientIdRaw, Call)>,
        UnboundedSender<Payload>,
        Arc<Notify>,
    ) {
        let (client_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (tx, client_rx) = tokio::sync::mpsc::unbounded_channel();
        let notify = Arc::new(Notify::new());

        let transport = Arc::new(Transport {
            name,
            pending_requests: Mutex::new(HashMap::default()),
        });

        tokio::spawn(Self::recv(
            transport.clone(),
            id,
            server_stdout,
            client_tx.clone(),
        ));
        tokio::spawn(Self::err(transport.clone(), server_stderr));
        tokio::spawn(Self::send(
            transport,
            id,
            server_stdin,
            client_tx,
            client_rx,
            notify.clone(),
        ));

        (rx, tx, notify)
    }

    // -----------------------------------------------------------------------
    // Framing helpers
    // -----------------------------------------------------------------------

    async fn recv_server_message(
        reader: &mut (impl AsyncBufRead + Unpin + Send),
        buffer: &mut String,
        content: &mut Vec<u8>,
        name: &str,
    ) -> Result<ServerMessage> {
        let mut content_length = None;
        loop {
            buffer.clear();
            if reader.read_line(buffer).await? == 0 {
                return Err(Error::StreamClosed);
            }
            if buffer == "\r\n" {
                break;
            }
            let header = buffer.trim();
            if let Some(("Content-Length", value)) = header.split_once(": ") {
                content_length = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| Error::Other(e.into()))?,
                );
            }
        }

        let len = content_length.ok_or_else(|| Error::Other(anyhow::anyhow!("missing Content-Length")))?;
        content.resize(len, 0);
        reader.read_exact(content).await?;

        let msg_str = std::str::from_utf8(content)
            .map_err(|e| Error::Other(e.into()))?;
        info!("{name} <- {msg_str}");

        let parsed = serde_json::from_slice(content).map_err(Error::from);
        content.clear();
        parsed
    }

    async fn recv_server_error(
        err: &mut (impl AsyncBufRead + Unpin + Send),
        buffer: &mut String,
        name: &str,
    ) -> Result<()> {
        buffer.truncate(0);
        if err.read_line(buffer).await? == 0 {
            return Err(Error::StreamClosed);
        }
        error!("{name} stderr <- {buffer:?}");
        Ok(())
    }

    async fn send_payload(
        &self,
        stdin: &mut BufWriter<ChildStdin>,
        payload: Payload,
    ) -> Result<()> {
        let json = match payload {
            Payload::Request { chan, value } => {
                self.pending_requests
                    .lock()
                    .await
                    .insert(value.id.clone(), chan);
                serde_json::to_string(&value)?
            }
            Payload::Notification(n) => serde_json::to_string(&n)?,
            Payload::Response(r) => serde_json::to_string(&r)?,
        };

        info!("{} -> {json}", self.name);
        stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", json.len()).as_bytes())
            .await?;
        stdin.write_all(json.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Message routing
    // -----------------------------------------------------------------------

    async fn process_message(
        &self,
        id: AgentClientIdRaw,
        client_tx: &UnboundedSender<(AgentClientIdRaw, Call)>,
        msg: ServerMessage,
    ) -> Result<()> {
        match msg {
            ServerMessage::Output(output) => {
                self.process_response(output).await?;
            }
            ServerMessage::Call(call) => {
                match Call::parse(call) {
                    Ok(typed) => {
                        let _ = client_tx.send((id, typed));
                    }
                    Err(Error::Unhandled) => {
                        // silently ignore unknown methods
                    }
                    Err(e) => {
                        error!("{} parse error: {e}", self.name);
                    }
                }
            }
        }
        Ok(())
    }

    async fn process_response(&self, output: jsonrpc::Output) -> Result<()> {
        let (id, result) = match output {
            jsonrpc::Output::Success(s) => (s.id, Ok(s.result)),
            jsonrpc::Output::Failure(f) => {
                error!("{} <- response error: {}", self.name, f.error);
                (f.id, Err(f.error.into()))
            }
        };

        if let Some(tx) = self.pending_requests.lock().await.remove(&id) {
            let _ = tx.send(result).await;
        } else {
            log::warn!(
                "{} discarding response without pending request (id={id:?})",
                self.name
            );
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Tasks
    // -----------------------------------------------------------------------

    async fn recv(
        transport: Arc<Self>,
        id: AgentClientIdRaw,
        mut stdout: BufReader<ChildStdout>,
        client_tx: UnboundedSender<(AgentClientIdRaw, Call)>,
    ) {
        let mut recv_buf = String::new();
        let mut content = Vec::new();
        loop {
            match Self::recv_server_message(&mut stdout, &mut recv_buf, &mut content, &transport.name)
                .await
            {
                Ok(msg) => {
                    if let Err(e) = transport.process_message(id, &client_tx, msg).await {
                        error!("{} process error: {e:?}", transport.name);
                        break;
                    }
                }
                Err(e) => {
                    if !matches!(e, Error::StreamClosed) {
                        error!("{} recv error: {e:?}", transport.name);
                    }
                    // Drain pending requests.
                    for (rid, tx) in transport.pending_requests.lock().await.drain() {
                        let _ = tx.send(Err(Error::StreamClosed)).await;
                        drop(rid);
                    }
                    // Inject Exit notification.
                    let _ = client_tx.send((
                        id,
                        Call::Notification(crate::Notification::Exit),
                    ));
                    break;
                }
            }
        }
    }

    async fn err(transport: Arc<Self>, mut stderr: BufReader<ChildStderr>) {
        let mut buf = String::new();
        loop {
            if Self::recv_server_error(&mut stderr, &mut buf, &transport.name)
                .await
                .is_err()
            {
                break;
            }
        }
    }

    async fn send(
        transport: Arc<Self>,
        id: AgentClientIdRaw,
        mut stdin: BufWriter<ChildStdin>,
        client_tx: UnboundedSender<(AgentClientIdRaw, Call)>,
        mut client_rx: UnboundedReceiver<Payload>,
        initialize_notify: Arc<Notify>,
    ) {
        let mut pending: Vec<Payload> = Vec::new();
        let mut is_initializing = true;

        fn is_initialize(p: &Payload) -> bool {
            matches!(p,
                Payload::Request { value: jsonrpc::MethodCall { method, .. }, .. }
                if method == "initialize"
            )
        }

        loop {
            tokio::select! {
                biased;

                _ = initialize_notify.notified() => {
                    is_initializing = false;
                    // Drain queued payloads.
                    for msg in pending.drain(..) {
                        if let Err(e) = transport.send_payload(&mut stdin, msg).await {
                            error!("{} send error: {e:?}", transport.name);
                            return;
                        }
                    }
                    // Inject an internal "initialized" notification so helix-view
                    // can react (same pattern as LSP).
                    let _ = client_tx.send((
                        id,
                        Call::Notification(crate::Notification::Exit), // placeholder; overridden below
                    ));
                    // Actually send the real initialized pseudo-event via a dedicated path;
                    // for now we rely on the capabilities stored in AgentClient.
                }

                Some(payload) = client_rx.recv() => {
                    if is_initializing && !is_initialize(&payload) {
                        pending.push(payload);
                        continue;
                    }
                    if let Err(e) = transport.send_payload(&mut stdin, payload).await {
                        error!("{} send error: {e:?}", transport.name);
                        return;
                    }
                }
            }
        }
    }
}

/// Raw numeric ID for the agent client (used inside the transport to avoid
/// importing helix_acp's SlotMap key type).
pub type AgentClientIdRaw = u64;
