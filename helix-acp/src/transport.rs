//! ACP transport: newline-delimited JSON over stdio (no Content-Length).

use crate::jsonrpc::{self, Call, MethodCall, Notification, Output};
use crate::{Error, Result};
use log::info;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::{ChildStderr, ChildStdin, ChildStdout},
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex, Notify,
    },
};

pub type AgentId = slotmap::DefaultKey;

#[derive(Debug)]
pub enum Payload {
    Request {
        chan: tokio::sync::mpsc::Sender<Result<Value>>,
        value: MethodCall,
    },
    Notification(Notification),
    Response(Output),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ServerMessage {
    Output(Output),
    Call(Call),
}

pub struct Transport {
    id: AgentId,
    name: String,
    pending_requests: Mutex<HashMap<jsonrpc::Id, tokio::sync::mpsc::Sender<Result<Value>>>>,
}

impl Transport {
    pub fn start(
        server_stdout: BufReader<ChildStdout>,
        server_stdin: BufWriter<ChildStdin>,
        server_stderr: BufReader<ChildStderr>,
        id: AgentId,
        name: String,
    ) -> (
        UnboundedReceiver<(AgentId, Call)>,
        UnboundedSender<Payload>,
        Arc<Notify>,
    ) {
        let (client_tx, rx) = unbounded_channel();
        let (tx, client_rx) = unbounded_channel();
        let notify = Arc::new(Notify::new());

        let transport = Arc::new(Self {
            id,
            name,
            pending_requests: Mutex::new(HashMap::new()),
        });

        tokio::spawn(Self::recv(
            transport.clone(),
            server_stdout,
            client_tx.clone(),
        ));
        tokio::spawn(Self::err(transport.clone(), server_stderr));
        tokio::spawn(Self::send(transport, server_stdin, client_tx, client_rx, notify.clone()));

        (rx, tx, notify)
    }

    /// Receive one newline-delimited JSON message.
    async fn recv_server_message(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin + Send),
        buffer: &mut String,
        name: &str,
    ) -> Result<ServerMessage> {
        buffer.clear();
        if reader.read_line(buffer).await? == 0 {
            return Err(Error::StreamClosed);
        }
        let line = buffer.trim_end_matches(|c| c == '\r' || c == '\n');
        if line.is_empty() {
            return Err(Error::StreamClosed);
        }
        info!("{name} <- {line}");
        let msg: ServerMessage = serde_json::from_str(line).map_err(Error::from)?;
        Ok(msg)
    }

    async fn recv_stderr(
        err: &mut (impl tokio::io::AsyncBufRead + Unpin + Send),
        buffer: &mut String,
        name: &str,
    ) -> Result<()> {
        buffer.clear();
        if err.read_line(buffer).await? == 0 {
            return Err(Error::StreamClosed);
        }
        log::error!("{name} stderr <- {}", buffer.trim_end());
        Ok(())
    }

    async fn send_payload(
        &self,
        server_stdin: &mut BufWriter<ChildStdin>,
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
            Payload::Response(o) => serde_json::to_string(&o)?,
        };
        info!("{} -> {}", self.name, json);
        server_stdin.write_all(json.as_bytes()).await?;
        server_stdin.write_all(b"\n").await?;
        server_stdin.flush().await?;
        Ok(())
    }

    async fn process_server_message(
        &self,
        client_tx: &UnboundedSender<(AgentId, Call)>,
        msg: ServerMessage,
    ) -> Result<()> {
        match msg {
            ServerMessage::Output(output) => {
                let (id, result) = match &output {
                    jsonrpc::Output::Success(s) => (s.id.clone(), Ok(s.result.clone())),
                    jsonrpc::Output::Failure(f) => {
                        log::error!("{} <- {}", self.name, f.error);
                        (f.id.clone(), Err(Error::Rpc(f.error.clone())))
                    }
                };
                if let Some(tx) = self.pending_requests.lock().await.remove(&id) {
                    let _ = tx.send(result).await;
                } else {
                    log::error!("Discarding ACP response without request (id={id:?})");
                }
            }
            ServerMessage::Call(call) => {
                client_tx.send((self.id, call)).map_err(|_| Error::StreamClosed)?;
            }
        }
        Ok(())
    }

    async fn recv(
        transport: Arc<Self>,
        mut server_stdout: BufReader<ChildStdout>,
        client_tx: UnboundedSender<(AgentId, Call)>,
    ) {
        let mut buffer = String::new();
        loop {
            match Self::recv_server_message(&mut server_stdout, &mut buffer, &transport.name).await
            {
                Ok(msg) => {
                    if let Err(e) = transport.process_server_message(&client_tx, msg).await {
                        log::error!("{} recv error: {e:?}", transport.name);
                        break;
                    }
                }
                Err(Error::StreamClosed) => break,
                Err(e) => {
                    log::error!("{} recv error: {e:?}", transport.name);
                    for (id, tx) in transport.pending_requests.lock().await.drain() {
                        let _ = tx.send(Err(Error::StreamClosed)).await;
                        log::debug!("Closed pending request {id:?}");
                    }
                    break;
                }
            }
        }
    }

    async fn err(transport: Arc<Self>, mut server_stderr: BufReader<ChildStderr>) {
        let mut buffer = String::new();
        loop {
            if Self::recv_stderr(&mut server_stderr, &mut buffer, &transport.name)
                .await
                .is_err()
            {
                break;
            }
        }
    }

    async fn send(
        transport: Arc<Self>,
        mut server_stdin: BufWriter<ChildStdin>,
        _client_tx: UnboundedSender<(AgentId, Call)>,
        mut client_rx: UnboundedReceiver<Payload>,
        initialize_notify: Arc<Notify>,
    ) {
        let mut pending = Vec::new();
        let mut is_pending = true;

        fn is_initialize(payload: &Payload) -> bool {
            match payload {
                Payload::Request {
                    value: MethodCall { method, .. },
                    ..
                } => method == "initialize",
                Payload::Notification(Notification { method, .. }) => method == "initialized",
                _ => false,
            }
        }

        loop {
            tokio::select! {
                _ = initialize_notify.notified() => {
                    is_pending = false;
                    for msg in pending.drain(..) {
                        if let Err(e) = transport.send_payload(&mut server_stdin, msg).await {
                            log::error!("{} send error: {e:?}", transport.name);
                        }
                    }
                }
                msg = client_rx.recv() => {
                    if let Some(msg) = msg {
                        if is_pending && !is_initialize(&msg) {
                            if matches!(&msg, Payload::Notification(_)) {
                                continue;
                            }
                            pending.push(msg);
                        } else {
                            if let Err(e) = transport.send_payload(&mut server_stdin, msg).await {
                                log::error!("{} send error: {e:?}", transport.name);
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }
}
