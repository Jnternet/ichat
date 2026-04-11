use anyhow::bail;
use reqwest::Client;
use rustls::crypto::aws_lc_rs;
use sea_orm::Database;
use sea_orm::DatabaseConnection;
use sea_orm::TransactionTrait;
use sha2::Digest;
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
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

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

    let gu = GetUpdate {
        auth,
        last_known: Some("2026-04-11T07:30:57.649111800+00:00".parse()?),
    };
    let url = format!("https://{}/update_info", server_name);
    let r = update_info(&g_client, &url, &gu).await;
    dbg!(&r);

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
async fn save_to_db(db: &DatabaseConnection, nm: NewMessages) -> anyhow::Result<()> {
    let txn = db.begin().await?;
    //在事务内操作
    txn.commit().await?;
    Ok(())
}
