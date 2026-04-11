use sea_orm::{ActiveModelTrait, ConnectionTrait};
use shared::message::S2C_Msg;

use anyhow::bail;
use reqwest::Client;
use shared::group::GetGroup;
use shared::group::GetGroupResponse;
use shared::group::GetGroupSuccess;
use shared::group::GroupError;
use shared::serde_json;

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

pub async fn get_group(
    client: &Client,
    url: &str,
    get_group: &GetGroup,
) -> anyhow::Result<GetGroupResponse> {
    let text = client
        .post(url)
        .json(get_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<GetGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(GetGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(GetGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}
