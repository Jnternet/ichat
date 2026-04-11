use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{auth::Auth, message::S2C_Msg};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GetUpdate {
    pub auth: Auth,
    pub last_known: Option<DateTime<Utc>>,
}
impl GetUpdate {
    pub fn set_time(&mut self, time: DateTime<Utc>) {
        self.last_known = Some(time)
    }
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

    pub fn messages(&self) -> &Vec<S2C_Msg> {
        &self.messages
    }

    pub fn last_known(&self) -> &DateTime<Utc> {
        &self.last_known
    }
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum UpdateInfoError {
    #[error("this account has no permission")]
    NoPermission,
    #[error("no message newer than last_known")]
    NoNewMessage,
    #[error("ServerError")]
    ServerError,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UpdateInfoResponse {
    Success(NewMessages),
    Fail(UpdateInfoError),
}
impl UpdateInfoResponse {
    pub fn success(self) -> Option<NewMessages> {
        match self {
            Self::Success(s) => Some(s),
            Self::Fail(e) => {
                dbg!(&e);
                None
            }
        }
    }
}
