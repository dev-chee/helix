//! Registry for managing ACP agents

use crate::client::Client;
use crate::{Error, Result, StartupError};
use slotmap::SlotMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_stream::wrappers::UnboundedReceiverStream;

slotmap::new_key_type! {
    /// Unique identifier for an ACP agent
    pub struct AgentId;
}

/// Agent name type
pub type AgentName = String;

/// Configuration for an AI coding agent
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AgentConfiguration {
    /// Whether this agent is enabled
    #[serde(default = "default_agent_enabled")]
    pub enabled: bool,
    /// Command to run the agent
    pub command: String,
    /// Arguments to pass to the agent
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the agent
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub environment: HashMap<String, String>,
    /// Configuration to pass to the agent
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// Request timeout in seconds
    #[serde(default = "default_agent_timeout")]
    pub timeout: u64,
}

fn default_agent_enabled() -> bool {
    true
}

fn default_agent_timeout() -> u64 {
    60
}

impl Default for AgentConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            command: String::new(),
            args: Vec::new(),
            environment: HashMap::new(),
            config: None,
            timeout: 60,
        }
    }
}

/// Registry for managing connected ACP agents
#[derive(Debug)]
pub struct Registry {
    inner: SlotMap<AgentId, Arc<Client>>,
    inner_by_name: HashMap<AgentName, Vec<Arc<Client>>>,
    agent_configs: HashMap<String, AgentConfiguration>,
    pub incoming:
        futures_util::stream::SelectAll<UnboundedReceiverStream<(AgentId, crate::transport::Call)>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            inner: SlotMap::with_key(),
            inner_by_name: HashMap::new(),
            agent_configs: HashMap::new(),
            incoming: futures_util::stream::SelectAll::new(),
        }
    }

    /// Update agent configurations
    pub fn set_configurations(&mut self, configs: HashMap<String, AgentConfiguration>) {
        self.agent_configs = configs;
    }

    /// Get agent configuration by name
    pub fn get_configuration(&self, name: &str) -> Option<&AgentConfiguration> {
        self.agent_configs.get(name)
    }

    /// Get all agent configurations
    pub fn configurations(&self) -> &HashMap<String, AgentConfiguration> {
        &self.agent_configs
    }

    pub fn get_by_id(&self, id: AgentId) -> Option<&Arc<Client>> {
        self.inner.get(id)
    }

    pub fn remove_by_id(&mut self, id: AgentId) {
        let Some(client) = self.inner.remove(id) else {
            log::debug!("agent was already removed");
            return;
        };
        let instances = self
            .inner_by_name
            .get_mut(client.name())
            .expect("inner and inner_by_name must be synced");
        instances.retain(|agent| id != agent.id());
        if instances.is_empty() {
            self.inner_by_name.remove(client.name());
        }
    }

    fn start_client(
        &mut self,
        name: &str,
        root_path: PathBuf,
    ) -> Result<Arc<Client>, StartupError> {
        let agent_config = self.agent_configs
            .get(name)
            .ok_or_else(|| StartupError::Error(Error::Other(anyhow::anyhow!("Agent '{name}' not defined"))))?
            .clone();

        if !agent_config.enabled {
            return Err(StartupError::Error(Error::Other(anyhow::anyhow!("Agent '{name}' is disabled"))));
        }

        let id = self.inner.try_insert_with_key(|id| {
            start_client(id, name.to_string(), &agent_config, &root_path).map(
                |client| {
                    self.incoming.push(UnboundedReceiverStream::new(client.1));
                    client.0
                },
            )
        })?;

        Ok(self.inner[id].clone())
    }

    /// Restart an agent
    pub fn restart_agent(
        &mut self,
        name: &str,
        root_path: &PathBuf,
    ) -> Option<Result<Arc<Client>>> {
        if let Some(old_clients) = self.inner_by_name.remove(name) {
            if old_clients.is_empty() {
                log::info!("restarting agent '{}' which was manually stopped", name);
            } else {
                log::info!("stopping existing clients for '{}'", name);
            }
            for old_client in old_clients {
                self.inner.remove(old_client.id());
                tokio::spawn(async move {
                    let _ = old_client.force_shutdown().await;
                });
            }
        }

        let client = match self.start_client(name, root_path.clone()) {
            Ok(client) => client,
            Err(StartupError::NoRequiredRootFound) => return None,
            Err(StartupError::Error(err)) => return Some(Err(err)),
        };

        self.inner_by_name
            .insert(name.to_owned(), vec![client.clone()]);

        Some(Ok(client))
    }

    /// Stop an agent
    pub fn stop(&mut self, name: &str) {
        if let Some(clients) = self.inner_by_name.get_mut(name) {
            for client in clients.drain(..) {
                self.inner.remove(client.id());
                tokio::spawn(async move {
                    let _ = client.force_shutdown().await;
                });
            }
        }
    }

    /// Get or start an agent
    pub fn get(
        &mut self,
        name: &str,
        root_path: &PathBuf,
    ) -> Option<Result<Arc<Client>>> {
        if let Some(clients) = self.inner_by_name.get(name) {
            if clients.is_empty() {
                return None;
            }
            if let Some(client) = clients.first() {
                return Some(Ok(client.clone()));
            }
        }

        match self.start_client(name, root_path.clone()) {
            Ok(client) => {
                self.inner_by_name
                    .entry(name.to_owned())
                    .or_default()
                    .push(client.clone());
                Some(Ok(client))
            }
            Err(StartupError::NoRequiredRootFound) => None,
            Err(StartupError::Error(err)) => Some(Err(err)),
        }
    }

    pub fn iter_agents(&self) -> impl Iterator<Item = &Arc<Client>> {
        self.inner.values()
    }

    /// Check if there are any running agents
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get an agent by name (returns the first one if multiple exist)
    pub fn get_by_name(&self, name: &str) -> Option<Arc<Client>> {
        self.inner_by_name
            .get(name)
            .and_then(|clients| clients.first().cloned())
    }

    /// Stop all agents
    pub fn stop_all(&mut self) {
        for (_, clients) in self.inner_by_name.drain() {
            for client in clients {
                self.inner.remove(client.id());
                tokio::spawn(async move {
                    let _ = client.force_shutdown().await;
                });
            }
        }
    }
}

fn start_client(
    id: AgentId,
    name: String,
    agent_config: &AgentConfiguration,
    root_path: &PathBuf,
) -> Result<(Arc<Client>, UnboundedReceiver<(AgentId, crate::transport::Call)>), StartupError> {
    let (client, rx, _notify) = Client::start(
        &agent_config.command,
        &agent_config.args,
        agent_config.config.clone(),
        agent_config.environment.iter().map(|(k, v)| (k, v)),
        root_path.clone(),
        id,
        name,
        agent_config.timeout,
    )?;

    let client = Arc::new(client);
    let (tx, client_rx) = tokio::sync::mpsc::unbounded_channel();

    // Forward messages with agent ID
    tokio::spawn(async move {
        use tokio_stream::StreamExt;
        let mut rx = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        while let Some(call) = rx.next().await {
            if tx.send((id, call)).is_err() {
                break;
            }
        }
    });

    Ok((client, client_rx))
}
