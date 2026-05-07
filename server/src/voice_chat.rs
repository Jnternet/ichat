use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sea_orm::Database;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::{TlsAcceptor, TlsStream};

pub async fn run() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

    let server_addr = std::env::var("SERVER_VOICE_CHAT_ADDR")?;
    let listener = TcpListener::bind(server_addr).await?;

    let tls_acceptor = get_acceptor().await?;

    // let online_groups = OnlineGroups::new();

    loop {
        let (stream, addr) = listener.accept().await?;
        eprintln!("客户端连接: {}", addr);
        let tls_stream = tls_acceptor.accept(stream).await?;
        let tls_stream = TlsStream::from(tls_stream);

        let db_ = db.clone();
        // let online_groups_ = online_groups.clone();
        tokio::spawn(async move {
            // let r = handle_client(db_, tls_stream, online_groups_).await;
            // if r.is_err() {
            //     dbg!(&r);
            // }
        });
    }
}

pub async fn get_acceptor() -> anyhow::Result<TlsAcceptor> {
    let cert_path = std::env::var("CERT_PATH")?;
    let key_path = std::env::var("KEY_PATH")?;
    let certs = CertificateDer::pem_file_iter(cert_path)?
        .map(|cert| cert.unwrap())
        .collect::<Vec<_>>();
    let key = PrivateKeyDer::from_pem_file(key_path)?;

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    anyhow::Ok(TlsAcceptor::from(Arc::new(server_config)))
}
