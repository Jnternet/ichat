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

#[derive(Debug, Serialize, Deserialize)]
pub struct JoinGroup {
    pub auth: Auth,
    pub group_id: GroupId,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct JoinGroupSuccess;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExitGroup {
    pub auth: Auth,
    pub group_id: GroupId,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExitGroupSuccess;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteGroup {
    pub auth: Auth,
    pub group_id: GroupId,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteGroupSuccess;

#[derive(Debug, Serialize, Deserialize)]
pub struct ListGroups {
    pub auth: Auth,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ListGroupsSuccess {
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetGroup {
    pub auth: Auth,
    pub group_id: GroupId,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct GetGroupSuccess {
    pub group: Group,
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum GroupError {
    #[error("You have no permission to do this behavior")]
    NoPermission,
    #[error("Target group not exist")]
    GroupNotFound,
    #[error("UnKnown Error")]
    UnKnown,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CreateGroupResponse {
    Success(CreateGroupSuccess),
    Fail(GroupError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum JoinGroupResponse {
    Success(JoinGroupSuccess),
    Fail(GroupError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ExitGroupResponse {
    Success(ExitGroupSuccess),
    Fail(GroupError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DeleteGroupResponse {
    Success(DeleteGroupSuccess),
    Fail(GroupError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ListGroupsResponse {
    Success(ListGroupsSuccess),
    Fail(GroupError),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GetGroupResponse {
    Success(GetGroupSuccess),
    Fail(GroupError),
}
