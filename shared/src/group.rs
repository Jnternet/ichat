use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GroupId(pub uuid::Uuid);
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGroup {
    pub name: String,
}
