use crate::auth;
use crate::entity::{groups, messages};
use anyhow;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use shared::message::C2S_Msg;
use uuid;

const MAX_MESSAGE_LENGTH: usize = 1000;
/// 保存消息的函数
///
/// # 参数
/// - `db`: 数据库连接
/// - `msg`: 客户端发送的消息
///
/// # 返回值
/// - `Ok(())`: 消息保存成功
/// - `Err(MessageError::NoPermission)`: 权限验证失败
/// - `Err(MessageError::GroupNotFound)`: 目标群组不存在
/// - `Err(MessageError::UnKnown)`: 其他未知错误
pub async fn save_msg(db: &impl ConnectionTrait, msg: C2S_Msg) -> Result<(), MessageError> {
    // 1. 验证 token
    if !auth::auth(db, msg.auth()).await {
        return Err(MessageError::NoPermission);
    }

    // 2. 检查群组是否存在
    let group_entity = groups::Entity::find_by_id(msg.target().0)
        .one(db)
        .await
        .map_err(anyhow::Error::from)?;

    if group_entity.is_none() {
        return Err(MessageError::GroupNotFound);
    }

    // 3. 验证消息内容
    validate_message(&msg)?;

    // 4. 保存消息到数据库
    use crate::entity::messages;
    let new_message = messages::ActiveModel {
        uuid: Set(uuid::Uuid::now_v7()),
        content: Set(msg.msg().text().to_string()),
        account_uuid: Set(msg.auth().account_id()),
        group_uuid: Set(msg.target().0),
    };

    new_message.insert(db).await.map_err(anyhow::Error::from)?;

    Ok(())
}

/// 验证消息的函数
///
/// # 参数
/// - `msg`: 要验证的消息
///
/// # 返回值
/// - `Ok(())`: 消息验证成功
/// - `Err(MessageError::UnKnown)`: 消息验证失败
fn validate_message(msg: &C2S_Msg) -> Result<(), MessageError> {
    // 验证消息文本不为空
    if msg.msg().text().is_empty() {
        return Err(MessageError::UnKnown(anyhow::anyhow!(
            "Message text cannot be empty"
        )));
    }

    // 验证消息文本长度不超过最大限制（例如 1000 字符）
    if msg.msg().text().len() > MAX_MESSAGE_LENGTH {
        return Err(MessageError::UnKnown(anyhow::anyhow!(
            "Message text too long, maximum length is {MAX_MESSAGE_LENGTH} characters"
        )));
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("You have no permission to do this behavior")]
    NoPermission,
    #[error("Target group not exist")]
    GroupNotFound,
    #[error("UnKnown Error: {0}")]
    UnKnown(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono;
    use migration::MigratorTrait;
    use sea_orm::{ActiveModelTrait, Database, Set};
    use sha2::Digest;
    use shared::{auth::Auth, group::GroupId, message::Msg};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_save_msg_valid_message() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let auth = Auth::new(account_id, token);

        // 插入到数据库中
        let _m = crate::entity::accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = crate::entity::auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 创建一个群组
        let group_name = "Test Group";
        let new_group = groups::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            group_name: Set(group_name.to_string()),
            create_at: Set(chrono::Utc::now()),
        };
        let group = new_group.insert(&db).await?;

        // 准备消息数据
        let target = GroupId(group.uuid);
        let msg = Msg::new("Hello, world!".to_string());
        let c2s_msg = C2S_Msg::new(auth, target, msg);

        // 执行测试
        let result = save_msg(&db, c2s_msg).await;

        // 验证结果
        assert!(result.is_ok());

        // 验证消息是否成功保存到数据库
        let saved_messages = crate::entity::messages::Entity::find()
            .filter(crate::entity::messages::Column::GroupUuid.eq(group.uuid))
            .filter(crate::entity::messages::Column::AccountUuid.eq(account_id))
            .all(&db)
            .await?;

        assert_eq!(saved_messages.len(), 1, "应该只保存了一条消息");
        assert_eq!(saved_messages[0].content, "Hello, world!", "消息内容不匹配");

        Ok(())
    }

    #[tokio::test]
    async fn test_save_msg_no_permission() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let invalid_token = "invalid_token_12345";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let auth = Auth::new(account_id, invalid_token);

        // 插入到数据库中
        let _m = crate::entity::accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = crate::entity::auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 创建一个群组
        let group_name = "Test Group";
        let new_group = groups::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            group_name: Set(group_name.to_string()),
            create_at: Set(chrono::Utc::now()),
        };
        let group = new_group.insert(&db).await?;

        // 准备消息数据
        let target = GroupId(group.uuid);
        let msg = Msg::new("Hello, world!".to_string());
        let c2s_msg = C2S_Msg::new(auth, target, msg);

        // 执行测试
        let result = save_msg(&db, c2s_msg).await;

        // 验证结果
        assert!(result.is_err());
        match result.unwrap_err() {
            MessageError::NoPermission => {}
            _ => panic!("Expected NoPermission error"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_save_msg_group_not_found() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let auth = Auth::new(account_id, token);

        // 插入到数据库中
        let _m = crate::entity::accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = crate::entity::auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 使用不存在的群组 ID
        let target = GroupId(Uuid::new_v4());
        let msg = Msg::new("Hello, world!".to_string());
        let c2s_msg = C2S_Msg::new(auth, target, msg);

        // 执行测试
        let result = save_msg(&db, c2s_msg).await;

        // 验证结果
        assert!(result.is_err());
        match result.unwrap_err() {
            MessageError::GroupNotFound => {}
            _ => panic!("Expected GroupNotFound error"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_save_msg_empty_message() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let auth = Auth::new(account_id, token);

        // 插入到数据库中
        let _m = crate::entity::accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = crate::entity::auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 创建一个群组
        let group_name = "Test Group";
        let new_group = groups::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            group_name: Set(group_name.to_string()),
            create_at: Set(chrono::Utc::now()),
        };
        let group = new_group.insert(&db).await?;

        // 准备空消息数据
        let target = GroupId(group.uuid);
        let msg = Msg::new("".to_string());
        let c2s_msg = C2S_Msg::new(auth, target, msg);

        // 执行测试
        let result = save_msg(&db, c2s_msg).await;

        // 验证结果
        assert!(result.is_err());
        match result.unwrap_err() {
            MessageError::UnKnown(err) => {
                assert!(err.to_string().contains("Message text cannot be empty"));
            }
            _ => panic!("Expected UnKnown error for empty message"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_save_msg_max_length_message() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let auth = Auth::new(account_id, token);

        // 插入到数据库中
        let _m = crate::entity::accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = crate::entity::auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 创建一个群组
        let group_name = "Test Group";
        let new_group = groups::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            group_name: Set(group_name.to_string()),
            create_at: Set(chrono::Utc::now()),
        };
        let group = new_group.insert(&db).await?;

        // 准备超长消息数据
        let target = GroupId(group.uuid);
        let long_text = "a".repeat(1001);
        let msg = Msg::new(long_text);
        let c2s_msg = C2S_Msg::new(auth, target, msg);

        // 执行测试
        let result = save_msg(&db, c2s_msg).await;

        // 验证结果
        assert!(result.is_err());
        match result.unwrap_err() {
            MessageError::UnKnown(err) => {
                assert!(err.to_string().contains("Message text too long"));
            }
            _ => panic!("Expected UnKnown error for long message"),
        }

        Ok(())
    }
}
