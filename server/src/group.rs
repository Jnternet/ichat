use crate::auth;
use crate::entity::groups;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use shared::group::CreateGroup;
use shared::{auth::Auth, group::Group};

pub async fn create_group(
    db: &impl ConnectionTrait,
    auth: Auth,
    group: CreateGroup,
) -> Result<(), GroupError> {
    // 1. 验证 token
    if !auth::auth(db, &auth).await {
        return Err(GroupError::NoPermission);
    }

    let new_group = groups::ActiveModel {
        uuid: Set(uuid::Uuid::now_v7()),
        group_name: Set(group.name),
        create_at: Set(Utc::now()),
    };

    // 插入 group（如果 id 已存在会失败）
    new_group.insert(db).await.map_err(anyhow::Error::from)?;
    Ok(())
}

pub async fn join_group(
    db: &impl ConnectionTrait,
    auth: Auth,
    group: Group,
) -> Result<(), GroupError> {
    todo!()
}
pub async fn exit_group(
    db: &impl ConnectionTrait,
    auth: Auth,
    group: Group,
) -> Result<(), GroupError> {
    todo!()
}

pub async fn delete_group(
    db: &impl ConnectionTrait,
    auth: Auth,
    group: Group,
) -> Result<(), GroupError> {
    todo!()
}

#[derive(Debug, thiserror::Error)]
pub enum GroupError {
    #[error("You have no permission to do this behavior")]
    NoPermission,
    #[error("Target group not exist")]
    GroupNotFound,
    #[error("UnKnown Error: {0}")]
    UnKnown(#[from] anyhow::Error),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::entity::{accounts, auths, groups};
    use migration::MigratorTrait;
    use sea_orm::{ActiveModelTrait, Database, EntityTrait};
    use sha2::Digest;

    #[tokio::test]
    async fn test_create_group_success() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let au = Auth::new(account_id, token);
        let group_name = "Test Group";
        let group_data = CreateGroup { name: group_name.to_string() };

        // 插入到数据库中
        let _m = accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 执行创建群组操作
        let result = super::create_group(&db, au, group_data).await;
        assert!(result.is_ok(), "创建群组失败: {:?}", result);

        // 验证群组是否成功创建
        let groups = groups::Entity::find().all(&db).await?;
        assert_eq!(groups.len(), 1, "应该只创建了一个群组");
        assert_eq!(groups[0].group_name, group_name, "群组名称不匹配");

        Ok(())
    }

    #[tokio::test]
    async fn test_create_group_no_permission() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let invalid_token = "invalid_token_12345";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let au = Auth::new(account_id, invalid_token);
        let group_data = CreateGroup { name: "Test Group".to_string() };

        // 插入到数据库中
        let _m = accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 执行创建群组操作
        let result = super::create_group(&db, au, group_data).await;
        assert!(result.is_err(), "应该返回权限错误");
        match result.unwrap_err() {
            GroupError::NoPermission => {},
            _ => panic!("应该返回 NoPermission 错误"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_create_group_database_error() -> anyhow::Result<()> {
        dotenv::dotenv().ok();
        // 准备数据库
        let server_db_url = "sqlite::memory:";
        let db = Database::connect(server_db_url).await?;
        migration::Migrator::up(&db, None).await?;

        // 准备测试数据
        let token = "ddda6ea7f0ad4e98b689b96431fb5926";
        let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
        let au = Auth::new(account_id, token);

        // 插入到数据库中
        let _m = accounts::ActiveModel {
            uuid: Set(account_id),
            user_name: Set("123".to_string()),
            account: Set("123".to_string()),
            password: Set(sha2::Sha256::digest("123").as_slice().into()),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;
        let _a = auths::ActiveModel {
            token: Set(token.parse()?),
            account: Set(account_id),
            create_at: Set(chrono::Utc::now()),
        }
        .insert(&db)
        .await?;

        // 首先创建一个群组
        let first_group = CreateGroup {
            name: "Test Group".to_string(),
        };
        let result = super::create_group(&db, au, first_group).await;
        assert!(result.is_ok(), "第一次创建群组失败: {:?}", result);

        // 验证群组是否成功创建
        let groups = groups::Entity::find().all(&db).await?;
        assert_eq!(groups.len(), 1, "应该只创建了一个群组");

        Ok(())
    }
}
