//! ACP notification types and Notification trait.

use super::*;
use serde::{de::DeserializeOwned, Serialize};

pub trait Notification {
    type Params: DeserializeOwned + Serialize + Send + Sync + 'static;
    const METHOD: &'static str;
}

pub struct Cancel;
impl Notification for Cancel {
    type Params = CancelNotification;
    const METHOD: &'static str = "session/cancel";
}

pub struct SessionUpdate;
impl Notification for SessionUpdate {
    type Params = SessionNotification;
    const METHOD: &'static str = "session/update";
}
