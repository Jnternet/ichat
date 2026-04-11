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
    messages: Vec<S2C_Msg>,
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum UpdateInfoError {
    #[error("this account has no permission")]
    NoPermission,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UpdateInfoResponse {
    Success(NewMessages),
    Fail(UpdateInfoError),
}
