use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Auth {
    account_id: Uuid,
    token: String,
}
impl Auth {
    pub fn new(account_id: Uuid, token: &str) -> Self {
        Auth {
            account_id,
            token: token.into(),
        }
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn account_id(&self) -> Uuid {
        self.account_id
    }
}
