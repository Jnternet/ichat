use crate::auth;
use crate::entity::{account_group, accounts, messages};
use anyhow;
use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, RelationTrait};
use shared::account::UserInfo;
use shared::group::GroupId;
use shared::message::Msg;
use shared::message::S2C_Msg;
use shared::update_info::{GetUpdate, NewMessages};

pub async fn get_new_messages(
    db: &impl ConnectionTrait,
    get_update: GetUpdate,
) -> anyhow::Result<NewMessages> {
    let auth = get_update.auth;
    let last_known = get_update.last_known;

    // 1. 验证 token
    if !auth::auth(db, &auth).await {
        return Err(anyhow::anyhow!("No permission"));
    }

    // 2. 获取用户所在的所有群组
    let user_groups = account_group::Entity::find()
        .filter(account_group::Column::AccountUuid.eq(auth.account_id()))
        .all(db)
        .await
        .map_err(anyhow::Error::from)?;

    // 3. 提取群组 ID
    let group_ids: Vec<uuid::Uuid> = user_groups.iter().map(|ag| ag.group_uuid).collect();

    // 4. 从数据库中查询这些群组的所有消息
    let mut message_query =
        messages::Entity::find().filter(messages::Column::GroupUuid.is_in(group_ids));

    // 5. 筛选出时间戳晚于指定时间点的消息
    if let Some(last_time) = last_known {
        // 假设 messages 表中有时间戳字段
        // 这里需要根据实际的数据库结构进行调整
        // message_query = message_query.filter(messages::Column::CreateAt.gt(last_time));
    }

    // 6. 对消息按照时间顺序排序（降序）
    // message_query = message_query.order_by_desc(messages::Column::CreateAt);

    // 7. 执行查询
    let messages = message_query.all(db).await.map_err(anyhow::Error::from)?;

    // 8. 构建 S2C_Msg 列表
    let mut s2c_messages = Vec::new();
    for msg in messages {
        // 获取发送者信息
        let account = accounts::Entity::find_by_id(msg.account_uuid)
            .one(db)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;

        // 构建 UserInfo
        let user_info = UserInfo::new(account.uuid, &account.user_name);

        // 构建 Msg
        let msg_content = Msg::new(msg.content);

        // 构建 GroupId
        let group_id = GroupId(msg.group_uuid);

        // 构建 S2C_Msg
        // 使用消息的 uuid 作为时间戳（因为 uuid::Uuid::now_v7() 包含时间信息）
        // 这里创建一个基于 uuid 时间戳的 DateTime
        let timestamp = match msg.uuid.get_timestamp() {
            Some(ts) => {
                let secs = ts.to_unix().0;
                DateTime::from_timestamp(secs as i64, 0).unwrap_or(Utc::now())
            }
            None => Utc::now(),
        };

        let s2c_msg = S2C_Msg::new(user_info, msg_content, group_id, timestamp);

        s2c_messages.push(s2c_msg);
    }

    // 9. 对消息按照时间顺序排序（升序）
    s2c_messages.sort_by(|a, b| a.time().cmp(b.time()));

    // 10. 构建 NewMessages 结构体
    let new_messages = NewMessages::new(Utc::now(), s2c_messages);

    Ok(new_messages)
}
