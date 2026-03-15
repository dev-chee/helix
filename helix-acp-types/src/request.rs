//! ACP request types and Request trait.

use super::*;
use serde::{de::DeserializeOwned, Serialize};

pub trait Request {
    type Params: DeserializeOwned + Serialize + Send + Sync + 'static;
    type Result: DeserializeOwned + Serialize + Send + Sync + 'static;
    const METHOD: &'static str;
}

pub struct Initialize;
impl Request for Initialize {
    type Params = InitializeRequest;
    type Result = InitializeResponse;
    const METHOD: &'static str = "initialize";
}

pub struct Authenticate;
impl Request for Authenticate {
    type Params = serde_json::Value;
    type Result = serde_json::Value;
    const METHOD: &'static str = "authenticate";
}

pub struct NewSession;
impl Request for NewSession {
    type Params = NewSessionRequest;
    type Result = NewSessionResponse;
    const METHOD: &'static str = "session/new";
}

pub struct Prompt;
impl Request for Prompt {
    type Params = PromptRequest;
    type Result = PromptResponse;
    const METHOD: &'static str = "session/prompt";
}
