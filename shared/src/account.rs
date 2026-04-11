use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OtherUser {
    user_name: String,
}

impl OtherUser {
    pub fn new(user_name: String) -> OtherUser {
        OtherUser { user_name }
    }
    pub fn user_name(&self) -> &str {
        &self.user_name
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserInfo {
    account_id: AccountId,
    user_name: String,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash, Copy, Clone)]
pub struct AccountId(pub uuid::Uuid);

impl UserInfo {
    pub fn new(account_id: Uuid, user_name: &str) -> Self {
        Self {
            account_id: AccountId(account_id),
            user_name: user_name.to_owned(),
        }
    }
    pub fn user_name(&self) -> &str {
        &self.user_name
    }
    pub fn id(&self) -> Uuid {
        self.account_id.0
    }
}
