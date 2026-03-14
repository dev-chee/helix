use crate::keymap;
use crate::keymap::{merge_keys, KeyTrie};
use helix_loader::merge_toml_values;
use helix_view::{document::Mode, theme};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::io::Error as IOError;
use toml::de::Error as TomlError;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub theme: Option<theme::Config>,
    pub keys: HashMap<Mode, KeyTrie>,
    pub editor: helix_view::editor::Config,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigRaw {
    pub theme: Option<theme::Config>,
    pub keys: Option<HashMap<Mode, KeyTrie>>,
    pub editor: Option<toml::Value>,
    pub agent: Option<toml::Value>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            theme: None,
            keys: keymap::default(),
            editor: helix_view::editor::Config::default(),
        }
    }
}

#[derive(Debug)]
pub enum ConfigLoadError {
    BadConfig(TomlError),
    Error(IOError),
}

impl Default for ConfigLoadError {
    fn default() -> Self {
        ConfigLoadError::Error(IOError::new(std::io::ErrorKind::NotFound, "place holder"))
    }
}

impl Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::BadConfig(err) => err.fmt(f),
            ConfigLoadError::Error(err) => err.fmt(f),
        }
    }
}

impl Config {
    /// Merge editor and agent configurations from global and local configs
    fn merge_editor_config(
        global_editor: Option<toml::Value>,
        local_editor: Option<toml::Value>,
        global_agent: Option<toml::Value>,
        local_agent: Option<toml::Value>,
    ) -> Result<helix_view::editor::Config, ConfigLoadError> {
        // Merge editor configs
        let mut editor = match (global_editor, local_editor) {
            (None, None) => toml::Value::Table(toml::map::Map::new()),
            (None, Some(val)) | (Some(val), None) => val,
            (Some(global), Some(local)) => merge_toml_values(global, local, 3),
        };

        // Merge agent configs
        let agent = match (global_agent, local_agent) {
            (None, None) => None,
            (None, Some(val)) | (Some(val), None) => Some(val),
            (Some(global), Some(local)) => Some(merge_toml_values(global, local, 3)),
        };

        // Add agent to editor config if present
        if let Some(agent_value) = agent {
            if let toml::Value::Table(ref mut table) = editor {
                table.insert("agent".to_string(), agent_value);
            }
        }

        editor.try_into().map_err(ConfigLoadError::BadConfig)
    }

    pub fn load(
        global: Result<String, ConfigLoadError>,
        local: Result<String, ConfigLoadError>,
    ) -> Result<Config, ConfigLoadError> {
        let global_config: Result<ConfigRaw, ConfigLoadError> =
            global.and_then(|file| toml::from_str(&file).map_err(ConfigLoadError::BadConfig));
        let local_config: Result<ConfigRaw, ConfigLoadError> =
            local.and_then(|file| toml::from_str(&file).map_err(ConfigLoadError::BadConfig));
        let res = match (global_config, local_config) {
            (Ok(global), Ok(local)) => {
                let mut keys = keymap::default();
                if let Some(global_keys) = global.keys {
                    merge_keys(&mut keys, global_keys)
                }
                if let Some(local_keys) = local.keys {
                    merge_keys(&mut keys, local_keys)
                }

                let editor = Self::merge_editor_config(global.editor, local.editor, global.agent, local.agent)?;

                Config {
                    theme: local.theme.or(global.theme),
                    keys,
                    editor,
                }
            }
            // if any configs are invalid return that first
            (_, Err(ConfigLoadError::BadConfig(err)))
            | (Err(ConfigLoadError::BadConfig(err)), _) => {
                return Err(ConfigLoadError::BadConfig(err))
            }
            (Ok(config), Err(_)) | (Err(_), Ok(config)) => {
                let mut keys = keymap::default();
                if let Some(keymap) = config.keys {
                    merge_keys(&mut keys, keymap);
                }
                Config {
                    theme: config.theme,
                    keys,
                    editor: Self::merge_editor_config(config.editor, None, config.agent, None)?,
                }
            }

            // these are just two io errors return the one for the global config
            (Err(err), Err(_)) => return Err(err),
        };

        Ok(res)
    }

    pub fn load_default() -> Result<Config, ConfigLoadError> {
        let global_config =
            fs::read_to_string(helix_loader::config_file()).map_err(ConfigLoadError::Error);
        let local_config = fs::read_to_string(helix_loader::workspace_config_file())
            .map_err(ConfigLoadError::Error);
        Config::load(global_config, local_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Config {
        fn load_test(config: &str) -> Config {
            Config::load(Ok(config.to_owned()), Err(ConfigLoadError::default())).unwrap()
        }
    }

    #[test]
    fn parsing_keymaps_config_file() {
        use crate::keymap;
        use helix_core::hashmap;
        use helix_view::document::Mode;

        let sample_keymaps = r#"
            [keys.insert]
            y = "move_line_down"
            S-C-a = "delete_selection"

            [keys.normal]
            A-F12 = "move_next_word_end"
        "#;

        let mut keys = keymap::default();
        merge_keys(
            &mut keys,
            hashmap! {
                Mode::Insert => keymap!({ "Insert mode"
                    "y" => move_line_down,
                    "S-C-a" => delete_selection,
                }),
                Mode::Normal => keymap!({ "Normal mode"
                    "A-F12" => move_next_word_end,
                }),
            },
        );

        assert_eq!(
            Config::load_test(sample_keymaps),
            Config {
                keys,
                ..Default::default()
            }
        );
    }

    #[test]
    fn keys_resolve_to_correct_defaults() {
        // From serde default
        let default_keys = Config::load_test("").keys;
        assert_eq!(default_keys, keymap::default());

        // From the Default trait
        let default_keys = Config::default().keys;
        assert_eq!(default_keys, keymap::default());
    }

    #[test]
    fn parsing_agent_config() {
        let config = r#"
            [agent]
            iflow = { command = "iflow", args = ["--experimental-acp"], timeout = 120 }
        "#;

        let loaded = Config::load_test(config);
        assert!(loaded.editor.agent.contains_key("iflow"));
        let iflow_config = loaded.editor.agent.get("iflow").unwrap();
        assert_eq!(iflow_config.command, "iflow");
        assert_eq!(iflow_config.args, vec!["--experimental-acp"]);
        assert_eq!(iflow_config.timeout, 120);
    }
}
