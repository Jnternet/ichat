use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Auth {
    account_id: String,
    token: String,
}
impl Auth {
    pub fn new(account_id: &str, token: &str) -> Self {
        Auth {
            account_id: account_id.into(),
            token: token.into(),
        }
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn account_id(&self) -> &str {
        &self.account_id
    }
}
