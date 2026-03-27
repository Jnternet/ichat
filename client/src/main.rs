use bytes::BytesMut;
use shared::*;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{TlsConnector, TlsStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let server_addr = std::env::var("SERVER_SOCK_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    let connector = get_connector();
    let mut tls_stream = get_tls_stream(&connector, &server_addr, &server_name).await?;

    let t = Test::new("client msg".to_string());
    let buf = rkyv::to_bytes::<rkyv::rancor::Error>(&t)?;
    tls_stream.write_all(&buf).await?;
    tls_stream.flush().await?;
    let mut buf = BytesMut::with_capacity(4096);
    tls_stream.read_buf(&mut buf).await?;
    println!("服务器消息：{}", std::str::from_utf8(&buf)?);

    std::thread::park();
    anyhow::Ok(())
}

pub async fn get_tls_stream(
    connector: &TlsConnector,
    server_addr: impl AsRef<str>,
    server_name: impl AsRef<str>,
) -> anyhow::Result<TlsStream<tokio::net::TcpStream>> {
    let server_addr = server_addr.as_ref();
    let server_name = server_name.as_ref().to_owned();
    let tcp = tokio::net::TcpStream::connect(&server_addr).await?;
    let stream = connector.connect(server_name.try_into()?, tcp).await?;
    anyhow::Ok(TlsStream::from(stream))
}

pub fn get_connector() -> TlsConnector {
    let root_cert_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(client_config))
}
