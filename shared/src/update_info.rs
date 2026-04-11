use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{auth::Auth, message::S2C_Msg};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetUpdate {
    pub auth: Auth,
    pub last_known: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewMessages {
    last_known: DateTime<Utc>,
    messages: Vec<S2C_Msg>,
}

impl NewMessages {
    pub fn new(last_known: DateTime<Utc>, messages: Vec<S2C_Msg>) -> Self {
        Self {
            last_known,
            messages,
        }
    }
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum UpdateInfoError {
    #[error("this account has no permission")]
    NoPermission,
    #[error("no message newer than last_known")]
    NoNewMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UpdateInfoResponse {
    Success(NewMessages),
    Fail(UpdateInfoError),
}
