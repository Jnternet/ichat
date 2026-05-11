use crate::tools::update_info::save_msg;
use sea_orm::DatabaseConnection;
use shared::auth::Auth;
use shared::message::C2S_Msg;
use shared::*;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio_rustls::{TlsConnector, TlsStream};
use tracing::{debug, error, info, instrument};

#[instrument(skip(db, recv, send))]
pub async fn text_chat(
    auth: Auth,
    db: DatabaseConnection,
    mut recv: Receiver<C2S_Msg>,
    send: Sender<()>,
) -> anyhow::Result<()> {
    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    info!(
        "[textchat] Connecting to server: {} ({})",
        server_addr, server_name
    );

    let connector = get_connector();
    let mut tls_stream = match get_tls_stream(&connector, &server_addr, &server_name).await {
        Ok(s) => {
            info!("[textchat] TLS connection established");
            s
        }
        Err(e) => {
            error!("[textchat] TLS connection failed: {:?}", e);
            return Err(e);
        }
    };

    let auth_json = serde_json::to_vec(&auth)?;
    debug!("[textchat] Sending auth ({} bytes)", auth_json.len());
    tls_stream.write_all(&auth_json).await?;
    tls_stream.flush().await?;
    info!("[textchat] Auth sent, splitting read/write streams");

    let (read_half, write_half) = tokio::io::split(tls_stream);
    let db_ = db.clone();

    tokio::spawn(async move {
        info!("[textchat] Receive task started");
        let mut read_half = read_half;
        let mut buf = bytes::BytesMut::with_capacity(1024);
        let s = send;
        loop {
            match read_half.read_buf(&mut buf).await {
                Ok(0) => {
                    info!("[textchat] Server closed connection");
                    break;
                }
                Ok(n) => {
                    debug!("[textchat] Received {} bytes, total buf: {}", n, buf.len());
                    let msg = serde_json::from_slice::<shared::message::S2C_Msg>(&buf);
                    match msg {
                        Ok(s2c_msg) => {
                            debug!("[textchat] Message parsed successfully, saving to db");
                            buf.clear();
                            match save_msg(&db_, &s2c_msg).await {
                                Ok(_) => {
                                    let _ = s.send(()).await;
                                }
                                Err(e) => {
                                    error!("[textchat] Failed to save message: {:?}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            debug!(
                                "[textchat] Failed to parse message (buf {} bytes): {:?}",
                                buf.len(),
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("[textchat] Read failed: {:?}", e);
                    break;
                }
            }
        }
        info!("[textchat] Receive task exiting");
    });

    tokio::spawn(async move {
        info!("[textchat] Send task started");
        let mut wh = write_half;
        while let Some(msg) = recv.recv().await {
            let b = serde_json::to_vec(&msg).unwrap();
            debug!("[textchat] Sending message {} bytes", b.len());
            match wh.write_all(&b).await {
                Ok(_) => {}
                Err(e) => {
                    error!("[textchat] Send failed: {:?}", e);
                    break;
                }
            }
            match wh.flush().await {
                Ok(_) => {
                    debug!("[textchat] Message sent and flushed");
                }
                Err(e) => {
                    error!("[textchat] Flush failed: {:?}", e);
                    break;
                }
            }
        }
        info!("[textchat] Send task exiting");
    });

    Ok(())
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
