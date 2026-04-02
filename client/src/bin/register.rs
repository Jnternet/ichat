use anyhow::bail;
use reqwest::Client;
use rustls::crypto::aws_lc_rs;
use sha2::Digest;
use shared::register::*;
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
    let server_addr = std::env::var("SERVER_REGISTER_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;

    let client = reqwest::Client::builder()
        .resolve(&server_name, server_addr.parse()?)
        .tls_backend_preconfigured(client_config)
        .no_proxy()
        .build()?;

    let url = format!("https://{}/register", server_name);

    let password = sha2::Sha256::digest("123");
    let register_example = Register {
        user_name: "123".to_string(),
        account: "123".to_string(),
        password: password.as_slice().into(),
    };
    let res = register(&client, &url, &register_example).await;
    dbg!(&res);

    std::thread::park();
    anyhow::Ok(())
}
async fn register(
    client: &Client,
    url: &str,
    register: &Register,
) -> anyhow::Result<RegisterResponse> {
    let text = client.post(url).json(register).send().await?.text().await?;
    let result = serde_json::from_str::<RegisterSuccess>(&text);
    if let Ok(s) = result {
        return Ok(RegisterResponse::Success(s));
    }
    let result = serde_json::from_str::<RegisterError>(&text);
    if let Ok(e) = result {
        return Ok(RegisterResponse::Fail(e));
    }
    bail!("cannot resolve response")
}
