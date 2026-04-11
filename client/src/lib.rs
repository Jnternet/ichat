use sea_orm::{ActiveModelTrait, ConnectionTrait};
use shared::message::S2C_Msg;

pub mod entity;

pub async fn save_msg(db: &impl ConnectionTrait, msg: &S2C_Msg) -> anyhow::Result<()> {
    // 构建消息模型
    let new_message = entity::messages::ActiveModel {
        uuid: sea_orm::Set(msg.msg_id()),
        content: sea_orm::Set(msg.msg().text().to_string()),
        account_uuid: sea_orm::Set(msg.sender().id()),
        group_uuid: sea_orm::Set(msg.target().0),
        create_at: sea_orm::Set(*msg.time()),
    };

    // 插入消息
    new_message.insert(db).await.map_err(anyhow::Error::from)?;

    Ok(())
}
