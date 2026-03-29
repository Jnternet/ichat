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
    let url = format!("https://{}/h", server_name);

    // let server_addr = "127.0.0.1:9520";
    let client = reqwest::Client::builder()
        .resolve(&server_name, server_addr.parse()?)
        .tls_backend_preconfigured(client_config)
        .no_proxy()
        .build()?;
    let response = client.get(&url).send().await?;
    let body = response.text().await?;
    println!("服务器：{}", body);

    std::thread::park();
    anyhow::Ok(())
}
