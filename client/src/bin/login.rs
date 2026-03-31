use anyhow::bail;
use reqwest::Client;
use shared::login::*;
use shared::serde_json;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let root_cert_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let server_addr = std::env::var("SERVER_SOCK_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;

    let client = reqwest::Client::builder()
        .resolve(&server_name, server_addr.parse()?)
        .tls_backend_preconfigured(client_config)
        .no_proxy()
        .build()?;

    let url = format!("https://{}/login", server_name);
    let login_example = Login {
        user_name: "123".to_string(),
        password: "1232".to_string(),
    };
    let res = login(&client, &url, &login_example).await;
    dbg!(&res);

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
