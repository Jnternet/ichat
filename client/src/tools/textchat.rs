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

pub async fn text_chat(
    auth: Auth,
    db: DatabaseConnection,
    mut recv: Receiver<C2S_Msg>,
    send: Sender<()>,
) -> anyhow::Result<()> {
    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    eprintln!("[textchat] 连接服务器: {} ({})", server_addr, server_name);

    let connector = get_connector();
    let mut tls_stream = match get_tls_stream(&connector, &server_addr, &server_name).await {
        Ok(s) => { eprintln!("[textchat] TLS 连接成功"); s }
        Err(e) => { eprintln!("[textchat] TLS 连接失败: {:?}", e); return Err(e); }
    };

    let auth_json = serde_json::to_vec(&auth)?;
    eprintln!("[textchat] 发送 Auth ({} bytes)", auth_json.len());
    tls_stream.write_all(&auth_json).await?;
    tls_stream.flush().await?;
    eprintln!("[textchat] Auth 发送完毕，分离读写流");

    let (read_half, write_half) = tokio::io::split(tls_stream);
    let db_ = db.clone();

    // 接收消息的任务
    tokio::spawn(async move {
        eprintln!("[textchat] 接收任务启动");
        let mut read_half = read_half;
        let mut buf = bytes::BytesMut::with_capacity(1024);
        let s = send;
        loop {
            match read_half.read_buf(&mut buf).await {
                Ok(0) => {
                    eprintln!("[textchat] 服务器关闭连接");
                    break;
                }
                Ok(n) => {
                    eprintln!("[textchat] 收到 {} bytes，buf 总长 {}: {:?}", n, buf.len(), &buf[..]);
                    let msg = serde_json::from_slice::<shared::message::S2C_Msg>(&buf);
                    match msg {
                        Ok(s2c_msg) => {
                            eprintln!("[textchat] 解析消息成功，保存到 db");
                            buf.clear();
                            match save_msg(&db_, &s2c_msg).await {
                                Ok(_) => { s.send(()).await.unwrap(); }
                                Err(e) => { eprintln!("[textchat] 保存消息失败: {:?}", e); break; }
                            }
                        }
                        Err(e) => {
                            eprintln!("[textchat] 解析消息失败 (buf {} bytes): {:?}", buf.len(), e);
                            // 数据不完整时不清空，等待更多数据；若已确认损坏则清空
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[textchat] 读取失败: {:?}", e);
                    break;
                }
            }
        }
        eprintln!("[textchat] 接收任务退出");
    });

    // 发送消息的任务
    tokio::spawn(async move {
        eprintln!("[textchat] 发送任务启动");
        let mut wh = write_half;
        while let Some(msg) = recv.recv().await {
            let b = serde_json::to_vec(&msg).unwrap();
            eprintln!("[textchat] 发送消息 {} bytes", b.len());
            match wh.write_all(&b).await {
                Ok(_) => {}
                Err(e) => { eprintln!("[textchat] 发送失败: {:?}", e); break; }
            }
            match wh.flush().await {
                Ok(_) => { eprintln!("[textchat] 消息发送并 flush 完毕"); }
                Err(e) => { eprintln!("[textchat] flush 失败: {:?}", e); break; }
            }
        }
        eprintln!("[textchat] 发送任务退出");
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
