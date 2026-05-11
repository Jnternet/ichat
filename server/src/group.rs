use crate::auth;
use crate::entity::{account_group, groups};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use shared::account::AccountId;
use shared::group::Group;
use shared::group::{
    CreateGroup, CreateGroupSuccess, DeleteGroup, DeleteGroupSuccess, ExitGroup, ExitGroupSuccess,
    GetGroup, GetGroupSuccess, GroupId, JoinGroup, JoinGroupSuccess, ListGroups, ListGroupsSuccess,
};
use tracing::{debug, error, info, instrument, warn};

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use crate::axum::AppState;

#[instrument(skip(state))]
pub async fn route_create_group(
    State(state): State<AppState>,
    Json(cg): Json<CreateGroup>,
) -> Result<impl IntoResponse, GroupError> {
    info!("Create group request for account: {}", cg.auth.account_id());
    match create_group(&state.db, cg).await {
        Ok(gid) => {
            info!("Group created successfully: {}", gid.0);
            Ok(Json(CreateGroupSuccess { group_id: gid }))
        }
        Err(e) => {
            warn!("Create group failed: {:?}", e);
            Err(e)
        }
    }
}

#[instrument(skip(db))]
pub async fn create_group(
    db: &impl ConnectionTrait,
    group: CreateGroup,
) -> Result<GroupId, GroupError> {
    let auth = group.auth;

    if !auth::auth(db, &auth).await {
        warn!(
            "Create group: No permission for account {}",
            auth.account_id()
        );
        return Err(GroupError::NoPermission);
    }

    let new_group = groups::ActiveModel {
        uuid: Set(uuid::Uuid::now_v7()),
        group_name: Set(group.name.clone()),
        create_at: Set(Utc::now()),
    };

    let g = new_group.insert(db).await.map_err(|e| {
        error!("Failed to insert group: {:?}", e);
        anyhow::Error::from(e)
    })?;
    debug!("Group inserted: {} ({})", g.uuid, group.name);
    Ok(GroupId(g.uuid))
}

#[instrument(skip(state))]
pub async fn route_join_group(
    State(state): State<AppState>,
    Json(jg): Json<JoinGroup>,
) -> Result<impl IntoResponse, GroupError> {
    info!(
        "Join group request: account={}, group={}",
        jg.auth.account_id(),
        jg.group_id.0
    );
    match join_group(&state.db, jg).await {
        Ok(jgs) => {
            info!(
                "Joined group successfully: account={}, group={}",
                jgs.uid.0, jgs.gid.0
            );
            Ok(Json(jgs))
        }
        Err(e) => {
            warn!("Join group failed: {:?}", e);
            Err(e)
        }
    }
}

#[instrument(skip(db))]
pub async fn join_group(
    db: &impl ConnectionTrait,
    jg: JoinGroup,
) -> Result<JoinGroupSuccess, GroupError> {
    let auth = jg.auth;
    let group_id = jg.group_id;

    if !auth::auth(db, &auth).await {
        warn!(
            "Join group: No permission for account {}",
            auth.account_id()
        );
        return Err(GroupError::NoPermission);
    }

    let group_entity = groups::Entity::find_by_id(group_id.0)
        .one(db)
        .await
        .map_err(|e| {
            error!("Failed to check group existence: {:?}", e);
            anyhow::Error::from(e)
        })?;

    if group_entity.is_none() {
        warn!("Join group: Group not found {}", group_id.0);
        return Err(GroupError::GroupNotFound);
    }

    let existing = account_group::Entity::find()
        .filter(account_group::Column::AccountUuid.eq(auth.account_id()))
        .filter(account_group::Column::GroupUuid.eq(group_id.0))
        .one(db)
        .await
        .map_err(|e| {
            error!("Failed to check membership: {:?}", e);
            anyhow::Error::from(e)
        })?;

    if existing.is_some() {
        debug!(
            "Join group: Already in group {}, account {}",
            group_id.0,
            auth.account_id()
        );
        return Ok(JoinGroupSuccess {
            gid: group_id,
            uid: AccountId(auth.account_id()),
        });
    }

    let new_account_group = account_group::ActiveModel {
        account_uuid: Set(auth.account_id()),
        group_uuid: Set(group_id.0),
        ..Default::default()
    };

    new_account_group.insert(db).await.map_err(|e| {
        error!("Failed to insert membership: {:?}", e);
        anyhow::Error::from(e)
    })?;

    Ok(JoinGroupSuccess {
        gid: group_id,
        uid: AccountId(auth.account_id()),
    })
}
#[instrument(skip(state))]
pub async fn route_exit_group(
    State(state): State<AppState>,
    Json(eg): Json<ExitGroup>,
) -> Result<impl IntoResponse, GroupError> {
    info!(
        "Exit group request: account={}, group={}",
        eg.auth.account_id(),
        eg.group_id.0
    );
    let a = eg.auth.account_id();
    let gid = eg.group_id.0;
    match exit_group(&state.db, eg).await {
        Ok(_) => {
            info!("Exited group successfully: account={}, group={}", a, gid);
            Ok(Json(ExitGroupSuccess))
        }
        Err(e) => {
            warn!("Exit group failed: {:?}", e);
            Err(e)
        }
    }
}

#[instrument(skip(db))]
pub async fn exit_group(db: &impl ConnectionTrait, eg: ExitGroup) -> Result<(), GroupError> {
    let auth = eg.auth;
    let group_id = eg.group_id;

    if !auth::auth(db, &auth).await {
        warn!(
            "Exit group: No permission for account {}",
            auth.account_id()
        );
        return Err(GroupError::NoPermission);
    }

    let group_entity = groups::Entity::find_by_id(group_id.0)
        .one(db)
        .await
        .map_err(|e| {
            error!("Failed to check group existence: {:?}", e);
            anyhow::Error::from(e)
        })?;

    if group_entity.is_none() {
        warn!("Exit group: Group not found {}", group_id.0);
        return Err(GroupError::GroupNotFound);
    }

    let existing = account_group::Entity::find()
        .filter(account_group::Column::AccountUuid.eq(auth.account_id()))
        .filter(account_group::Column::GroupUuid.eq(group_id.0))
        .one(db)
        .await
        .map_err(|e| {
            error!("Failed to check membership: {:?}", e);
            anyhow::Error::from(e)
        })?;

    if existing.is_none() {
        debug!(
            "Exit group: Not in group {}, account {}",
            group_id.0,
            auth.account_id()
        );
        return Ok(());
    }

    let _delete_result = account_group::Entity::delete_many()
        .filter(account_group::Column::AccountUuid.eq(auth.account_id()))
        .filter(account_group::Column::GroupUuid.eq(group_id.0))
        .exec(db)
        .await
        .map_err(|e| {
            error!("Failed to remove membership: {:?}", e);
            anyhow::Error::from(e)
        })?;

    Ok(())
}

#[instrument(skip(state))]
pub async fn route_delete_group(
    State(state): State<AppState>,
    Json(dg): Json<DeleteGroup>,
) -> Result<impl IntoResponse, GroupError> {
    info!(
        "Delete group request: account={}, group={}",
        dg.auth.account_id(),
        dg.group_id.0
    );
    let gid = dg.group_id;
    match delete_group(&state.db, dg).await {
        Ok(_) => {
            info!("Deleted group successfully: {}", gid.0);
            Ok(Json(DeleteGroupSuccess))
        }
        Err(e) => {
            warn!("Delete group failed: {:?}", e);
            Err(e)
        }
    }
}

#[instrument(skip(db))]
pub async fn delete_group(db: &impl ConnectionTrait, dg: DeleteGroup) -> Result<(), GroupError> {
    let auth = dg.auth;
    let group_id = dg.group_id;

    if !auth::auth(db, &auth).await {
        warn!(
            "Delete group: No permission for account {}",
            auth.account_id()
        );
        return Err(GroupError::NoPermission);
    }

    let group_entity = groups::Entity::find_by_id(group_id.0)
        .one(db)
        .await
        .map_err(|e| {
            error!("Failed to check group existence: {:?}", e);
            anyhow::Error::from(e)
        })?;

    if group_entity.is_none() {
        warn!("Delete group: Group not found {}", group_id.0);
        return Err(GroupError::GroupNotFound);
    }

    groups::Entity::delete_by_id(group_id.0)
        .exec(db)
        .await
        .map_err(|e| {
            error!("Failed to delete group: {:?}", e);
            anyhow::Error::from(e)
        })?;

    Ok(())
}

#[instrument(skip(state))]
pub async fn route_list_groups(
    State(state): State<AppState>,
    Json(lg): Json<ListGroups>,
) -> Result<impl IntoResponse, GroupError> {
    info!("List groups request for account: {}", lg.auth.account_id());
    match list_groups(&state.db, lg).await {
        Ok(groups) => {
            info!("List groups successful, found {} groups", groups.len());
            Ok(Json(ListGroupsSuccess { groups }))
        }
        Err(e) => {
            warn!("List groups failed: {:?}", e);
            Err(e)
        }
    }
}

#[instrument(skip(db))]
pub async fn list_groups(
    db: &impl ConnectionTrait,
    lg: ListGroups,
) -> Result<Vec<Group>, GroupError> {
    let auth = lg.auth;

    if !auth::auth(db, &auth).await {
        warn!(
            "List groups: No permission for account {}",
            auth.account_id()
        );
        return Err(GroupError::NoPermission);
    }

    let account_groups = account_group::Entity::find()
        .filter(account_group::Column::AccountUuid.eq(auth.account_id()))
        .all(db)
        .await
        .map_err(|e| {
            error!("Failed to get account groups: {:?}", e);
            anyhow::Error::from(e)
        })?;

    let mut groups = Vec::new();
    for ag in account_groups {
        let group_entity = groups::Entity::find_by_id(ag.group_uuid)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to get group info: {:?}", e);
                anyhow::Error::from(e)
            })?;

        if let Some(group) = group_entity {
            groups.push(Group {
                id: GroupId(group.uuid),
                name: group.group_name,
            });
        }
    }

    Ok(groups)
}

#[instrument(skip(state))]
pub async fn route_get_group(
    State(state): State<AppState>,
    Json(gg): Json<GetGroup>,
) -> Result<impl IntoResponse, GroupError> {
    info!(
        "Get group request: account={}, group={}",
        gg.auth.account_id(),
        gg.group_id.0
    );
    match get_group(&state.db, gg).await {
        Ok(group) => {
            info!("Get group successful: {} ({})", group.id.0, group.name);
            Ok(Json(GetGroupSuccess { group }))
        }
        Err(e) => {
            warn!("Get group failed: {:?}", e);
            Err(e)
        }
    }
}

#[instrument(skip(db))]
pub async fn get_group(db: &impl ConnectionTrait, gg: GetGroup) -> Result<Group, GroupError> {
    let auth = gg.auth;
    let group_id = gg.group_id;

    if !auth::auth(db, &auth).await {
        warn!("Get group: No permission for account {}", auth.account_id());
        return Err(GroupError::NoPermission);
    }

    let group_entity = groups::Entity::find_by_id(group_id.0)
        .one(db)
        .await
        .map_err(|e| {
            error!("Failed to get group: {:?}", e);
            anyhow::Error::from(e)
        })?;

    if let Some(group) = group_entity {
        Ok(Group {
            id: GroupId(group.uuid),
            name: group.group_name,
        })
    } else {
        warn!("Get group: Group not found {}", group_id.0);
        Err(GroupError::GroupNotFound)
    }
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

impl IntoResponse for GroupError {
    fn into_response(self) -> axum::response::Response {
        match self {
            GroupError::NoPermission => (
                StatusCode::BAD_REQUEST,
                Json(shared::group::GroupError::NoPermission),
            )
                .into_response(),
            GroupError::GroupNotFound => (
                StatusCode::BAD_REQUEST,
                Json(shared::group::GroupError::GroupNotFound),
            )
                .into_response(),
            GroupError::UnKnown(_) => (
                StatusCode::BAD_REQUEST,
                Json(shared::group::GroupError::UnKnown),
            )
                .into_response(),
        }
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::entity::{accounts, auths, groups};
//     use migration::MigratorTrait;
//     use sea_orm::{ActiveModelTrait, ColumnTrait, Database, EntityTrait, QueryFilter};
//     use sha2::Digest;
//
//     #[tokio::test]
//     async fn test_create_group_success() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//         let group_name = "Test Group";
//         let group_data = CreateGroup {
//             name: group_name.to_string(),
//         };
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 执行创建群组操作
//         let result = super::create_group(&db, au, group_data).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 验证群组是否成功创建
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         assert_eq!(groups[0].group_name, group_name, "群组名称不匹配");
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_create_group_no_permission() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let invalid_token = "invalid_token_12345";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, invalid_token);
//         let group_data = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 执行创建群组操作
//         let result = super::create_group(&db, au, group_data).await;
//         assert!(result.is_err(), "应该返回权限错误");
//         match result.unwrap_err() {
//             GroupError::NoPermission => {}
//             _ => panic!("应该返回 NoPermission 错误"),
//         }
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_create_group_database_error() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 首先创建一个群组
//         let first_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let result = super::create_group(&db, au, first_group).await;
//         assert!(result.is_ok(), "第一次创建群组失败: {:?}", result);
//
//         // 验证群组是否成功创建
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_join_group_success() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let result = super::create_group(&db, au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 重新创建 auth 和 group 实例
//         let au2 = Auth::new(account_id, token);
//         let group2 = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 执行加入群组操作
//         let result = super::join_group(&db, au2, group2).await;
//         assert!(result.is_ok(), "加入群组失败: {:?}", result);
//
//         // 验证用户是否成功加入群组
//         use crate::entity::account_group;
//         let account_groups = account_group::Entity::find()
//             .filter(account_group::Column::AccountUuid.eq(account_id))
//             .filter(account_group::Column::GroupUuid.eq(group_id))
//             .all(&db)
//             .await?;
//         assert_eq!(account_groups.len(), 1, "用户应该成功加入群组");
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_join_group_no_permission() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let invalid_token = "invalid_token_12345";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, invalid_token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let valid_au = Auth::new(account_id, token);
//         let result = super::create_group(&db, valid_au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 执行加入群组操作
//         let result = super::join_group(&db, au, group).await;
//         assert!(result.is_err(), "应该返回权限错误");
//         match result.unwrap_err() {
//             GroupError::NoPermission => {}
//             _ => panic!("应该返回 NoPermission 错误"),
//         }
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_join_group_not_found() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 使用不存在的群组 ID
//         let non_existent_group_id = uuid::Uuid::new_v4();
//         let group = Group {
//             id: shared::group::GroupId(non_existent_group_id),
//             name: "Non-existent Group".to_string(),
//         };
//
//         // 执行加入群组操作
//         let result = super::join_group(&db, au, group).await;
//         assert!(result.is_err(), "应该返回群组不存在错误");
//         match result.unwrap_err() {
//             GroupError::GroupNotFound => {}
//             _ => panic!("应该返回 GroupNotFound 错误"),
//         }
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_join_group_already_in_group() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let result = super::create_group(&db, au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 重新创建 auth 实例
//         let au2 = Auth::new(account_id, token);
//
//         // 第一次加入群组
//         let result = super::join_group(&db, au2, group).await;
//         assert!(result.is_ok(), "第一次加入群组失败: {:?}", result);
//
//         // 重新创建 auth 和 group 实例
//         let au3 = Auth::new(account_id, token);
//         let group2 = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 第二次加入同一群组（应该成功，因为用户已经在群组中）
//         let result = super::join_group(&db, au3, group2).await;
//         assert!(result.is_ok(), "第二次加入群组失败: {:?}", result);
//
//         // 验证用户只在群组中一次
//         use crate::entity::account_group;
//         let account_groups = account_group::Entity::find()
//             .filter(account_group::Column::AccountUuid.eq(account_id))
//             .filter(account_group::Column::GroupUuid.eq(group_id))
//             .all(&db)
//             .await?;
//         assert_eq!(account_groups.len(), 1, "用户应该只在群组中一次");
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_exit_group_success() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let result = super::create_group(&db, au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 重新创建 auth 实例
//         let au2 = Auth::new(account_id, token);
//
//         // 先加入群组
//         let result = super::join_group(&db, au2, group).await;
//         assert!(result.is_ok(), "加入群组失败: {:?}", result);
//
//         // 验证用户是否成功加入群组
//         use crate::entity::account_group;
//         let account_groups = account_group::Entity::find()
//             .filter(account_group::Column::AccountUuid.eq(account_id))
//             .filter(account_group::Column::GroupUuid.eq(group_id))
//             .all(&db)
//             .await?;
//         assert_eq!(account_groups.len(), 1, "用户应该成功加入群组");
//
//         // 重新创建 auth 和 group 实例
//         let au3 = Auth::new(account_id, token);
//         let group3 = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 执行退出群组操作
//         let result = super::exit_group(&db, au3, group3).await;
//         assert!(result.is_ok(), "退出群组失败: {:?}", result);
//
//         // 验证用户是否成功退出群组
//         let account_groups = account_group::Entity::find()
//             .filter(account_group::Column::AccountUuid.eq(account_id))
//             .filter(account_group::Column::GroupUuid.eq(group_id))
//             .all(&db)
//             .await?;
//         assert_eq!(account_groups.len(), 0, "用户应该成功退出群组");
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_exit_group_no_permission() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let invalid_token = "invalid_token_12345";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, invalid_token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let valid_au = Auth::new(account_id, token);
//         let result = super::create_group(&db, valid_au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 执行退出群组操作
//         let result = super::exit_group(&db, au, group).await;
//         assert!(result.is_err(), "应该返回权限错误");
//         match result.unwrap_err() {
//             GroupError::NoPermission => {}
//             _ => panic!("应该返回 NoPermission 错误"),
//         }
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_exit_group_not_found() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 使用不存在的群组 ID
//         let non_existent_group_id = uuid::Uuid::new_v4();
//         let group = Group {
//             id: shared::group::GroupId(non_existent_group_id),
//             name: "Non-existent Group".to_string(),
//         };
//
//         // 执行退出群组操作
//         let result = super::exit_group(&db, au, group).await;
//         assert!(result.is_err(), "应该返回群组不存在错误");
//         match result.unwrap_err() {
//             GroupError::GroupNotFound => {}
//             _ => panic!("应该返回 GroupNotFound 错误"),
//         }
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_exit_group_not_in_group() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let result = super::create_group(&db, au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 重新创建 auth 实例
//         let au2 = Auth::new(account_id, token);
//
//         // 执行退出群组操作（用户不在群组中，应该成功）
//         let result = super::exit_group(&db, au2, group).await;
//         assert!(result.is_ok(), "退出群组失败: {:?}", result);
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_delete_group_success() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let result = super::create_group(&db, au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 重新创建 auth 实例
//         let au2 = Auth::new(account_id, token);
//
//         // 执行删除群组操作
//         let result = super::delete_group(&db, au2, group).await;
//         assert!(result.is_ok(), "删除群组失败: {:?}", result);
//
//         // 验证群组是否成功删除
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 0, "群组应该成功删除");
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_delete_group_no_permission() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let invalid_token = "invalid_token_12345";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, invalid_token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 创建一个群组
//         let create_group = CreateGroup {
//             name: "Test Group".to_string(),
//         };
//         let valid_au = Auth::new(account_id, token);
//         let result = super::create_group(&db, valid_au, create_group).await;
//         assert!(result.is_ok(), "创建群组失败: {:?}", result);
//
//         // 获取群组信息
//         let groups = groups::Entity::find().all(&db).await?;
//         assert_eq!(groups.len(), 1, "应该只创建了一个群组");
//         let group_id = groups[0].uuid;
//         let group = Group {
//             id: shared::group::GroupId(group_id),
//             name: groups[0].group_name.clone(),
//         };
//
//         // 执行删除群组操作
//         let result = super::delete_group(&db, au, group).await;
//         assert!(result.is_err(), "应该返回权限错误");
//         match result.unwrap_err() {
//             GroupError::NoPermission => {}
//             _ => panic!("应该返回 NoPermission 错误"),
//         }
//
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_delete_group_not_found() -> anyhow::Result<()> {
//         dotenv::dotenv().ok();
//         // 准备数据库
//         let server_db_url = "sqlite::memory:";
//         let db = Database::connect(server_db_url).await?;
//         migration::Migrator::up(&db, None).await?;
//
//         // 准备测试数据
//         let token = "ddda6ea7f0ad4e98b689b96431fb5926";
//         let account_id = "ad89ac437cf44ad1a85f47bfaa8c618a".parse()?;
//         let au = Auth::new(account_id, token);
//
//         // 插入到数据库中
//         let _m = accounts::ActiveModel {
//             uuid: Set(account_id),
//             user_name: Set("123".to_string()),
//             account: Set("123".to_string()),
//             password: Set(sha2::Sha256::digest("123").as_slice().into()),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//         let _a = auths::ActiveModel {
//             token: Set(token.parse()?),
//             account: Set(account_id),
//             create_at: Set(chrono::Utc::now()),
//         }
//         .insert(&db)
//         .await?;
//
//         // 使用不存在的群组 ID
//         let non_existent_group_id = uuid::Uuid::new_v4();
//         let group = Group {
//             id: shared::group::GroupId(non_existent_group_id),
//             name: "Non-existent Group".to_string(),
//         };
//
//         // 执行删除群组操作
//         let result = super::delete_group(&db, au, group).await;
//         assert!(result.is_err(), "应该返回群组不存在错误");
//         match result.unwrap_err() {
//             GroupError::GroupNotFound => {}
//             _ => panic!("应该返回 GroupNotFound 错误"),
//         }
//
//         Ok(())
//     }
// }
