//! Registry of ACP agents: multiple clients and a single merged incoming stream.

use crate::client::AcpClient;
use crate::jsonrpc::Call;
use crate::transport::AgentId;
use futures_util::stream::select_all::SelectAll;
use slotmap::SlotMap;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio_stream::wrappers::UnboundedReceiverStream;

/// Central registry of connected ACP agents. Maps agent id to client and name to clients; exposes merged incoming stream.
pub struct AgentRegistry {
    inner: SlotMap<AgentId, Arc<AcpClient>>,
    by_name: HashMap<String, Vec<Arc<AcpClient>>>,
    pub incoming: SelectAll<UnboundedReceiverStream<(AgentId, Call)>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            inner: SlotMap::with_key(),
            by_name: HashMap::new(),
            incoming: SelectAll::new(),
        }
    }

    /// Start and register one agent. Returns the client on success. Errors if `name` is already connected.
    pub fn start_agent(
        &mut self,
        name: String,
        command: &str,
        args: &[String],
        root_path: &Path,
    ) -> Result<Arc<AcpClient>, crate::Error> {
        if self.by_name.get(&name).map(|v| !v.is_empty()).unwrap_or(false) {
            return Err(crate::Error::Rpc(crate::jsonrpc::Error::invalid_params(
                "agent already connected",
            )));
        }
        let id = self.inner.try_insert_with_key(|id| -> Result<Arc<AcpClient>, crate::Error> {
            let (client, receiver, _notify) =
                AcpClient::start(command, args, root_path, id, name.clone())?;
            self.incoming.push(UnboundedReceiverStream::new(receiver));
            Ok(Arc::new(client))
        })?;
        let client = self.inner.get(id).cloned().unwrap();
        self.by_name
            .entry(name)
            .or_default()
            .push(client.clone());
        Ok(client)
    }

    pub fn get_by_id(&self, id: AgentId) -> Option<&Arc<AcpClient>> {
        self.inner.get(id)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&Arc<AcpClient>> {
        self.by_name.get(name).and_then(|v| v.first())
    }

    /// All connected agents (by name). A name may appear once.
    pub fn connected_names(&self) -> impl Iterator<Item = &String> {
        self.by_name.keys()
    }

    pub fn remove_by_id(&mut self, id: AgentId) -> Option<Arc<AcpClient>> {
        let client = self.inner.remove(id)?;
        let name = client.name().to_string();
        if let Some(list) = self.by_name.get_mut(&name) {
            list.retain(|c| c.id() != id);
            if list.is_empty() {
                self.by_name.remove(&name);
            }
        }
        Some(client)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<AcpClient>> {
        self.inner.values()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
