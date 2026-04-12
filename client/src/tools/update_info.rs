use crate::entity;
use crate::entity::accounts;
use crate::entity::groups;
use crate::entity::messages;
use crate::tools::group;
use anyhow::Context;
use anyhow::bail;
use reqwest::Client;
use sea_orm::ActiveModelTrait;
use sea_orm::ColumnTrait;
use sea_orm::EntityTrait;
use sea_orm::QueryFilter;
use sea_orm::TransactionTrait;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use shared::auth::Auth;
use shared::group::GetGroup;
use shared::group::GroupId;
use shared::message::S2C_Msg;
use shared::serde_json;
use shared::update_info::GetUpdate;
use shared::update_info::NewMessages;
use shared::update_info::UpdateInfoError;
use shared::update_info::UpdateInfoResponse;

pub async fn update_info(
    client: &Client,
    url: &str,
    get_update: &GetUpdate,
) -> anyhow::Result<UpdateInfoResponse> {
    let text = client
        .post(url)
        .json(get_update)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<NewMessages>(&text);
    if let Ok(s) = result {
        return Ok(UpdateInfoResponse::Success(s));
    }
    let result = serde_json::from_str::<UpdateInfoError>(&text);
    if let Ok(e) = result {
        return Ok(UpdateInfoResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

pub async fn save_to_db(
    db: &DatabaseConnection,
    client: &Client,
    url: &str,
    nm: NewMessages,
    auth: &Auth,
) -> anyhow::Result<()> {
    let txn = db.begin().await?;

    // 遍历所有消息并保存到数据库
    for msg in nm.messages() {
        // 检查并创建用户记录
        let account_id = msg.sender().id();
        let account = accounts::Entity::find_by_id(account_id)
            .one(&txn)
            .await
            .map_err(anyhow::Error::from)?;

        if account.is_none() {
            // 创建用户记录
            let new_account = accounts::ActiveModel {
                uuid: sea_orm::Set(account_id),
                user_name: sea_orm::Set(msg.sender().user_name().to_string()),
                account: sea_orm::Set(msg.sender().user_name().to_string()),
            };
            new_account
                .insert(&txn)
                .await
                .map_err(anyhow::Error::from)?;
        }

        // 检查并创建群组记录
        let group_id = msg.target().0;
        let group = groups::Entity::find_by_id(group_id)
            .one(&txn)
            .await
            .map_err(anyhow::Error::from)?;

        if group.is_none() {
            let get_group = GetGroup {
                auth: auth.clone(),
                group_id: GroupId(group_id),
            };
            let g = group::get_group(client, url, &get_group).await?;
            let g = g.success().context("Cannot get group info")?;

            // 创建群组记录
            let new_group = groups::ActiveModel {
                uuid: sea_orm::Set(group_id),
                group_name: sea_orm::Set(g.group.name), // 使用群组 ID 作为名称
            };
            new_group.insert(&txn).await.map_err(anyhow::Error::from)?;
        }

        // 保存消息
        let r = save_msg(&txn, msg).await;
        if r.is_err() {
            dbg!(&r);
        }
    }

    txn.commit().await?;
    Ok(())
}

/// 从数据库中查询当前用户最后一条信息的时间戳
///
/// # 参数
/// - `db`: 数据库连接
/// - `auth`: 用户认证信息
///
/// # 返回值
/// - `Ok(GetUpdate)`: 包含用户认证信息和最后一条消息的时间戳
/// - `Err(anyhow::Error)`: 数据库查询错误
pub async fn get_last_message_timestamp(
    db: &DatabaseConnection,
    auth: &shared::auth::Auth,
) -> anyhow::Result<GetUpdate> {
    // 查询用户的最后一条消息
    let last_message = messages::Entity::find()
        .filter(messages::Column::AccountUuid.eq(auth.account_id()))
        .order_by_id_desc()
        .one(db)
        .await
        .map_err(anyhow::Error::from)?;
    let last_known = last_message.map(|m| m.create_at);

    // 构建 GetUpdate
    let auth = auth.clone();

    Ok(GetUpdate { auth, last_known })
}
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
