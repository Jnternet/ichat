use serde::{Deserialize, Serialize};

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
