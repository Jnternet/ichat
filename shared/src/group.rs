use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    id: GroupId,
    name: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GroupId(uuid::Uuid);
