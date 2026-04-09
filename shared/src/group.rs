use serde::{Deserialize, Serialize};

use crate::auth::Auth;

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash, Copy, Clone)]
pub struct GroupId(pub uuid::Uuid);
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGroup {
    pub auth: Auth,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGroupSuccess;

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum GroupError {
    #[error("You have no permission to do this behavior")]
    NoPermission,
    #[error("Target group not exist")]
    GroupNotFound,
    #[error("UnKnown Error")]
    UnKnown,
}
