use anyhow::Context;
use anyhow::bail;
use reqwest::Client;
use rustls::crypto::aws_lc_rs;
use sea_orm::ActiveModelTrait;
use sea_orm::ColumnTrait;
use sea_orm::Database;
use sea_orm::DatabaseConnection;
use sea_orm::EntityTrait;
use sea_orm::TransactionTrait;
use sea_orm::prelude::DateTime;
use sha2::Digest;
use shared::auth::Auth;
use shared::chrono;
use shared::chrono::Utc;
use shared::group::GetGroup;
use shared::group::GroupId;
use shared::login::*;
use shared::serde_json;
use shared::update_info::GetUpdate;
use shared::update_info::NewMessages;
use shared::update_info::UpdateInfoError;
use shared::update_info::UpdateInfoResponse;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");

    //准备数据库
    let client_db_url = std::env::var("CLIENT_DATABASE")?;
    let db = Database::connect(client_db_url).await?;

    let root_cert_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let server_addr = std::env::var("SERVER_HTTPS_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;

    let client = reqwest::Client::builder()
        .resolve(&server_name, server_addr.parse()?)
        .tls_backend_preconfigured(client_config.clone())
        .no_proxy()
        .build()?;

    let url = format!("https://{}/login", server_name);
    let pwd = sha2::Sha256::digest("123");
    let login_example = Login {
        account: "123".to_string(),
        password: pwd.as_slice().into(),
    };
    let res = login(&client, &url, &login_example).await;
    dbg!(&res);

    let auth = res.unwrap().success().unwrap().auth;

    let server_addr = std::env::var("SERVER_HTTPS_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;

    let g_client = reqwest::Client::builder()
        .resolve(&server_name, server_addr.parse()?)
        .tls_backend_preconfigured(client_config.clone())
        .no_proxy()
        .build()?;
    let gu = get_last_message_timestamp(&db, &auth).await?;
    dbg!(&gu);

    let url = format!("https://{}/update_info", server_name);
    let r = update_info(&g_client, &url, &gu).await;
    dbg!(&r);

    let nm = r.unwrap().success().unwrap();
    let url = format!("https://{}/get_group", server_name);
    save_to_db(&db, &client, &url, nm, &auth).await?;

    std::thread::park();
    anyhow::Ok(())
}
async fn login(client: &Client, url: &str, login: &Login) -> anyhow::Result<LoginResponse> {
    let text = client.post(url).json(login).send().await?.text().await?;
    let result = serde_json::from_str::<LoginSuccess>(&text);
    if let Ok(s) = result {
        return Ok(LoginResponse::Success(s));
    }
    let result = serde_json::from_str::<LoginError>(&text);
    if let Ok(e) = result {
        return Ok(LoginResponse::Fail(e));
    }
    bail!("cannot resolve response")
}

async fn update_info(
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
async fn save_to_db(
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
        let account = client::entity::accounts::Entity::find_by_id(account_id)
            .one(&txn)
            .await
            .map_err(anyhow::Error::from)?;

        if account.is_none() {
            // 创建用户记录
            let new_account = client::entity::accounts::ActiveModel {
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
        let group = client::entity::groups::Entity::find_by_id(group_id)
            .one(&txn)
            .await
            .map_err(anyhow::Error::from)?;

        if group.is_none() {
            let get_group = GetGroup {
                auth: auth.clone(),
                group_id: GroupId(group_id),
            };
            let g = client::get_group(client, url, &get_group).await?;
            let g = g.success().context("Cannot get group info")?;

            // 创建群组记录
            let new_group = client::entity::groups::ActiveModel {
                uuid: sea_orm::Set(group_id),
                group_name: sea_orm::Set(g.group.name), // 使用群组 ID 作为名称
            };
            new_group.insert(&txn).await.map_err(anyhow::Error::from)?;
        }

        // 保存消息
        let r = client::save_msg(&txn, msg).await;
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
async fn get_last_message_timestamp(
    db: &DatabaseConnection,
    auth: &shared::auth::Auth,
) -> anyhow::Result<GetUpdate> {
    use sea_orm::{EntityTrait, QueryFilter};

    // 查询用户的最后一条消息
    let last_message = client::entity::messages::Entity::find()
        .filter(client::entity::messages::Column::AccountUuid.eq(auth.account_id()))
        .order_by_id_desc()
        .one(db)
        .await
        .map_err(anyhow::Error::from)?;
    let last_known = last_message.map(|m| m.create_at);

    // 构建 GetUpdate
    let auth = auth.clone();

    Ok(GetUpdate { auth, last_known })
}
