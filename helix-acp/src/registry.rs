//! Registry of `AgentClient` instances.
//!
//! Analogous to `helix_lsp::Registry`: holds all active agent clients and
//! provides a single merged stream of `(AgentClientId, Call)` events.

use crate::{client::AgentClientConfig, AgentClient, AgentClientId, Call, Result};
use futures_util::stream::select_all::SelectAll;
use slotmap::SlotMap;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub struct Registry {
    clients: SlotMap<AgentClientId, AgentClient>,
    /// All incoming (AgentClientId, Call) messages from every active client.
    pub incoming: SelectAll<UnboundedReceiverStream<(AgentClientId, Call)>>,
    /// Shared sender; each new client's transport posts here.
    call_tx: UnboundedSender<(AgentClientId, Call)>,
    call_rx: Option<UnboundedReceiver<(AgentClientId, Call)>>,
}

impl Registry {
    pub fn new() -> Self {
        let (call_tx, call_rx) = mpsc::unbounded_channel();
        Self {
            clients: SlotMap::with_key(),
            incoming: SelectAll::new(),
            call_tx,
            call_rx: Some(call_rx),
        }
    }

    /// Initialise the merged stream from the shared receiver.
    ///
    /// Call this once after construction to wire the receiver into `incoming`.
    pub fn init_incoming(&mut self) {
        if let Some(rx) = self.call_rx.take() {
            self.incoming
                .push(UnboundedReceiverStream::new(rx));
        }
    }

    /// Spawn an agent client from `config`, perform the `initialize` handshake
    /// asynchronously, and return its ID.
    pub fn start_client(&mut self, config: AgentClientConfig) -> Result<AgentClientId> {
        let id = self.clients.insert_with_key(|id| {
            AgentClient::start(id, &config, self.call_tx.clone())
                .expect("failed to spawn ACP agent")
        });
        // Ensure the registry exposes the shared channel through `incoming`.
        self.init_incoming();
        Ok(id)
    }

    pub fn get(&self, id: AgentClientId) -> Option<&AgentClient> {
        self.clients.get(id)
    }

    pub fn get_mut(&mut self, id: AgentClientId) -> Option<&mut AgentClient> {
        self.clients.get_mut(id)
    }

    pub fn remove(&mut self, id: AgentClientId) -> Option<AgentClient> {
        self.clients.remove(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (AgentClientId, &AgentClient)> {
        self.clients.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (AgentClientId, &mut AgentClient)> {
        self.clients.iter_mut()
    }

    /// Return the first client whose name matches `name`.
    pub fn by_name(&self, name: &str) -> Option<(AgentClientId, &AgentClient)> {
        self.clients
            .iter()
            .find(|(_, c)| c.name == name)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
