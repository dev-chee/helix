//! Transport layer for ACP communication over JSON-RPC

use crate::{Error, Result, RpcError};
use anyhow::Context;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
use tokio::sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender};

/// JSON-RPC version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V2,
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Version::V2 => serializer.serialize_str("2.0"),
        }
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        match s {
            "2.0" => Ok(Version::V2),
            _ => Err(serde::de::Error::custom("invalid version")),
        }
    }
}

/// Request ID type
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Null,
    Num(u64),
    Str(String),
}

/// JSON-RPC parameters
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Params {
    None,
    Array(Vec<Value>),
    Map(serde_json::Map<String, Value>),
}

impl Default for Params {
    fn default() -> Self {
        Params::None
    }
}

impl From<Value> for Params {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Params::None,
            Value::Bool(_) | Value::Number(_) | Value::String(_) => Params::Array(vec![value]),
            Value::Array(arr) => Params::Array(arr),
            Value::Object(map) => Params::Map(map),
        }
    }
}

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: Option<Version>,
    pub id: Id,
    pub method: String,
    #[serde(default, skip_serializing_if = "Params::is_none")]
    pub params: Params,
}

/// JSON-RPC notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: Option<Version>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Params::is_none")]
    pub params: Params,
}

impl Params {
    pub fn is_none(&self) -> bool {
        matches!(self, Params::None)
    }

    pub fn parse<T: for<'de> Deserialize<'de>>(self) -> Result<T> {
        let value: Value = self.into();
        serde_json::from_value(value).map_err(|e| Error::Parse(Box::new(e)))
    }
}

impl From<Params> for Value {
    fn from(params: Params) -> Self {
        match params {
            Params::Array(arr) => Value::Array(arr),
            Params::Map(map) => Value::Object(map),
            Params::None => Value::Null,
        }
    }
}

/// JSON-RPC success response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Success {
    pub jsonrpc: Option<Version>,
    pub result: Value,
    pub id: Id,
}

/// JSON-RPC error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    pub jsonrpc: Option<Version>,
    pub error: JsonRpcError,
    pub id: Id,
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC output (response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Output {
    Success(Success),
    Failure(Failure),
}

/// JSON-RPC call (request or notification)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Call {
    Request(Request),
    Notification(Notification),
}

/// Payload for transport
#[derive(Debug)]
pub enum Payload {
    Request {
        chan: Sender<Result<Value>>,
        value: Request,
    },
    Notification(Notification),
    Response(Output),
}

/// A message from the server
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum ServerMessage {
    Output(Output),
    Call(Call),
}

/// Transport handles JSON-RPC communication over stdio
#[derive(Debug)]
pub struct Transport {
    pending_requests: parking_lot::Mutex<HashMap<Id, Sender<Result<Value>>>>,
}

impl Transport {
    pub fn start(
        server_stdout: BufReader<ChildStdout>,
        server_stdin: BufWriter<ChildStdin>,
        server_stderr: BufReader<ChildStderr>,
        agent_name: &str,
    ) -> (
        UnboundedReceiver<Call>,
        UnboundedSender<Payload>,
        Arc<tokio::sync::Notify>,
    ) {
        let (client_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (tx, client_rx) = tokio::sync::mpsc::unbounded_channel();
        let notify = Arc::new(tokio::sync::Notify::new());

        let transport = Self {
            pending_requests: parking_lot::Mutex::new(HashMap::default()),
        };

        let transport = Arc::new(transport);
        let name = agent_name.to_string();

        tokio::spawn(Self::recv(transport.clone(), server_stdout, client_tx.clone(), name.clone()));
        tokio::spawn(Self::err(transport.clone(), server_stderr, name));
        tokio::spawn(Self::send(transport, server_stdin, client_tx, client_rx, notify.clone()));

        (rx, tx, notify)
    }

    async fn recv_server_message(
        reader: &mut (impl AsyncBufRead + Unpin + Send),
        buffer: &mut String,
        content: &mut Vec<u8>,
        agent_name: &str,
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
                content_length = Some(value.parse().context("invalid content length")?);
            }
        }

        let content_length = content_length.context("missing content length")?;
        content.resize(content_length, 0);
        reader.read_exact(content).await?;
        let msg = std::str::from_utf8(content).context("invalid utf8 from server")?;

        info!("{} <- {}", agent_name, msg);

        let output = sonic_rs::from_slice(content).map_err(Into::into);
        content.clear();
        output
    }

    async fn recv_server_error(
        err: &mut (impl AsyncBufRead + Unpin + Send),
        buffer: &mut String,
        agent_name: &str,
    ) -> Result<()> {
        buffer.truncate(0);
        if err.read_line(buffer).await? == 0 {
            return Err(Error::StreamClosed);
        }
        error!("{} err <- {:?}", agent_name, buffer);
        Ok(())
    }

    async fn send_payload_to_server(
        &self,
        server_stdin: &mut BufWriter<ChildStdin>,
        payload: Payload,
    ) -> Result<()> {
        let json = match payload {
            Payload::Request { chan, value } => {
                self.pending_requests.lock().insert(value.id.clone(), chan);
                serde_json::to_string(&value)?
            }
            Payload::Notification(value) => serde_json::to_string(&value)?,
            Payload::Response(output) => serde_json::to_string(&output)?,
        };
        self.send_string_to_server(server_stdin, json).await
    }

    async fn send_string_to_server(
        &self,
        server_stdin: &mut BufWriter<ChildStdin>,
        request: String,
    ) -> Result<()> {
        info!("-> {}", request);

        server_stdin
            .write_all(format!("Content-Length: {}\r\n\r\n", request.len()).as_bytes())
            .await?;
        server_stdin.write_all(request.as_bytes()).await?;
        server_stdin.flush().await?;

        Ok(())
    }

    async fn process_server_message(
        &self,
        client_tx: &UnboundedSender<Call>,
        msg: ServerMessage,
    ) -> Result<()> {
        match msg {
            ServerMessage::Output(output) => {
                self.process_request_response(output).await?;
            }
            ServerMessage::Call(call) => {
                client_tx.send(call).context("failed to send message")?;
            }
        }
        Ok(())
    }

    async fn process_request_response(&self, output: Output) -> Result<()> {
        let (id, result) = match output {
            Output::Success(success) => (success.id, Ok(success.result)),
            Output::Failure(failure) => {
                error!("<- error: {:?}", failure.error);
                (
                    failure.id,
                    Err(Error::Rpc(RpcError::JsonRpc {
                        code: failure.error.code,
                        message: failure.error.message,
                        data: failure.error.data,
                    })),
                )
            }
        };

        if let Some(tx) = self.pending_requests.lock().remove(&id) {
            match tx.send(result).await {
                Ok(_) => {}
                Err(_) => {
                    error!("Tried sending response into a closed channel (id={:?})", id);
                }
            }
        } else {
            error!("Discarding response without a request (id={:?})", id);
        }

        Ok(())
    }

    async fn recv(
        transport: Arc<Self>,
        mut server_stdout: BufReader<ChildStdout>,
        client_tx: UnboundedSender<Call>,
        agent_name: String,
    ) {
        let mut recv_buffer = String::new();
        let mut content_buffer = Vec::new();
        loop {
            match Self::recv_server_message(
                &mut server_stdout,
                &mut recv_buffer,
                &mut content_buffer,
                &agent_name,
            )
            .await
            {
                Ok(msg) => {
                    if let Err(err) = transport.process_server_message(&client_tx, msg).await {
                        error!("{} err: <- {:?}", agent_name, err);
                        break;
                    }
                }
                Err(err) => {
                    if !matches!(err, Error::StreamClosed) {
                        error!("Exiting {} after unexpected error: {:?}", agent_name, err);
                    }

                    // Close any outstanding requests
                    for (id, tx) in transport.pending_requests.lock().drain() {
                        match tx.send(Err(Error::StreamClosed)).await {
                            Ok(_) => {}
                            Err(_) => {
                                error!("Could not close request (id={:?})", id);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    async fn err(
        _transport: Arc<Self>,
        mut server_stderr: BufReader<ChildStderr>,
        agent_name: String,
    ) {
        let mut recv_buffer = String::new();
        loop {
            match Self::recv_server_error(&mut server_stderr, &mut recv_buffer, &agent_name).await {
                Ok(_) => {}
                Err(err) => {
                    error!("{} err: <- {:?}", agent_name, err);
                    break;
                }
            }
        }
    }

    async fn send(
        transport: Arc<Self>,
        mut server_stdin: BufWriter<ChildStdin>,
        _client_tx: UnboundedSender<Call>,
        mut client_rx: UnboundedReceiver<Payload>,
        _initialize_notify: Arc<tokio::sync::Notify>,
    ) {
        while let Some(msg) = client_rx.recv().await {
            if let Err(err) = transport.send_payload_to_server(&mut server_stdin, msg).await {
                error!("err: <- {:?}", err);
                break;
            }
        }
    }
}
