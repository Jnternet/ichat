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
    // 3. 建立与服务端的 TLS 连接
    let server_addr = std::env::var("SERVER_TEXTCHAT_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    let connector = get_connector();
    let mut tls_stream = get_tls_stream(&connector, &server_addr, &server_name).await?;

    // 4. 发送 Auth 信息进行认证
    let auth_json = serde_json::to_vec(&auth)?;
    tls_stream.write_all(&auth_json).await?;
    tls_stream.flush().await?;

    // 5. 分离读写流，分别处理消息的发送和接收
    let (read_half, write_half) = tokio::io::split(tls_stream);

    let db_ = db.clone();

    // 接收消息的任务
    tokio::spawn(async move {
        let mut read_half = read_half;
        let mut buf = bytes::BytesMut::with_capacity(1024);
        let s = send;
        loop {
            match read_half.read_buf(&mut buf).await {
                Ok(n) if n > 0 => {
                    let msg = serde_json::from_slice::<shared::message::S2C_Msg>(&buf[..n]);
                    match msg {
                        Ok(s2c_msg) => {
                            if save_msg(&db_, &s2c_msg).await.is_err() {
                                break;
                            };
                            // TODO: 此处应触发页面刷新
                            s.send(()).await.unwrap();
                        }
                        Err(e) => {
                            eprintln!("解析消息失败: {:?}", e);
                        }
                    }
                    // buf.clear();
                }
                Ok(_) => break,
                Err(e) => {
                    eprintln!("读取消息失败: {:?}", e);
                    break;
                }
            }
            // buf.clear();
        }
    });

    // 发送消息的任务
    tokio::spawn(async move {
        let mut wh = write_half;
        while let Some(msg) = recv.recv().await {
            let b = serde_json::to_vec(&msg).unwrap();
            wh.write_all(&b).await.unwrap();
            wh.flush().await.unwrap();
        }
    });

    //阻塞住主线程
    std::thread::park();
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
