use std::collections::VecDeque;

use sea_orm::{ConnectionTrait, DatabaseConnection};
use shared::group::GroupId;

pub(super) struct OneGroup {
    id: GroupId,
    name: String,
    last_msg: Option<String>,
}
pub(super) struct UIGroups {
    groups: VecDeque<OneGroup>,
}

pub(super) async fn get_groups_info(db: DatabaseConnection) -> UIGroups {
    todo!()
}
async fn get_one_group(db: impl ConnectionTrait, id: GroupId) -> OneGroup {
    todo!()
}
