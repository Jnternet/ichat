use anyhow::bail;
use reqwest::Client;
use rustls::crypto::aws_lc_rs;
use sha2::Digest;
use shared::auth::Auth;
use shared::group::CreateGroup;
use shared::group::CreateGroupResponse;
use shared::group::CreateGroupSuccess;
use shared::group::GroupError;
use shared::login::*;
use shared::serde_json;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");

    let root_cert_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let server_addr = std::env::var("SERVER_LOGIN_ADDR")?;
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

    let server_addr = std::env::var("SERVER_GROUP_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;

    let g_client = reqwest::Client::builder()
        .resolve(&server_name, server_addr.parse()?)
        .tls_backend_preconfigured(client_config.clone())
        .no_proxy()
        .build()?;

    let cg = CreateGroup {
        auth,
        name: "first_group".to_string(),
    };
    let url = format!("https://{}/create_group", server_name);
    let r = create_group(&g_client, &url, &cg).await;
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

async fn create_group(
    client: &Client,
    url: &str,
    create_group: &CreateGroup,
) -> anyhow::Result<CreateGroupResponse> {
    let text = client
        .post(url)
        .json(create_group)
        .send()
        .await?
        .text()
        .await?;
    let result = serde_json::from_str::<CreateGroupSuccess>(&text);
    if let Ok(s) = result {
        return Ok(CreateGroupResponse::Success(s));
    }
    let result = serde_json::from_str::<GroupError>(&text);
    if let Ok(e) = result {
        return Ok(CreateGroupResponse::Fail(e));
    }
    bail!("cannot resolve response")
}
