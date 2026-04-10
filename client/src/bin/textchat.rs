use rustls::crypto::aws_lc_rs;
use sha2::Digest;
use shared::*;
use std::io::{Write, stdin, stdout};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{TlsConnector, TlsStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    aws_lc_rs::default_provider()
        .install_default()
        .expect("unable to set aws_lc_rs as provider");

    // 1. 从命令行读取用户账号和密码
    println!("请输入账号：");
    let mut account = String::new();
    stdin().read_line(&mut account)?;
    account = account.trim().to_string();

    println!("请输入密码：");
    let mut password = String::new();
    stdin().read_line(&mut password)?;
    password = password.trim().to_string();

    // 2. 实现登录流程
    let auth = login(&account, &password).await?;
    println!("登录成功！");
    dbg!(&auth);

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

    // 6. 启动两个任务，分别处理消息的接收和发送
    let write_half = Arc::new(tokio::sync::Mutex::new(write_half));
    let write_half_clone = write_half.clone();

    // 接收消息的任务
    tokio::spawn(async move {
        let mut read_half = read_half;
        let mut buf = vec![0u8; 1024];
        loop {
            match read_half.read_buf(&mut buf).await {
                Ok(n) if n > 0 => {
                    let msg = serde_json::from_slice::<shared::message::S2C_Msg>(&buf[..n]);
                    match msg {
                        Ok(s2c_msg) => {
                            println!("\n[{}]: {}", s2c_msg.sender_name(), s2c_msg.msg().text());
                            print!("输入消息: ");
                            stdout().flush().unwrap();
                        }
                        Err(e) => {
                            eprintln!("解析消息失败: {:?}", e);
                        }
                    }
                    buf.clear();
                }
                Ok(_) => break,
                Err(e) => {
                    eprintln!("读取消息失败: {:?}", e);
                    break;
                }
            }
        }
    });

    // 发送消息的任务
    tokio::spawn(async move {
        let group_id = shared::group::GroupId(
            uuid::Uuid::parse_str("019d7621e9ad7ba3aa7274e811ae6bd9").unwrap(),
        );
        loop {
            print!("输入消息: ");
            stdout().flush().unwrap();
            let mut input = String::new();
            stdin().read_line(&mut input).unwrap();
            input = input.trim().to_string();

            if input == "exit" {
                break;
            }

            let msg = shared::message::Msg::new(input);
            let c2s_msg = shared::message::C2S_Msg::new(auth.clone(), group_id, msg);
            let msg_json = serde_json::to_vec(&c2s_msg).unwrap();

            let mut write_half = write_half_clone.lock().await;
            if let Err(e) = write_half.write_all(&msg_json).await {
                eprintln!("发送消息失败: {:?}", e);
                break;
            }
            if let Err(e) = write_half.flush().await {
                eprintln!("刷新缓冲区失败: {:?}", e);
                break;
            }
        }
    });

    // 等待用户输入 exit 退出
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

// 登录函数，返回 Auth 信息
async fn login(account: &str, password: &str) -> anyhow::Result<shared::auth::Auth> {
    dotenv::dotenv().ok();

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
    let pwd = sha2::Sha256::digest(password);
    let login_example = shared::login::Login {
        account: account.to_string(),
        password: pwd.as_slice().into(),
    };

    let res = client.post(&url).json(&login_example).send().await?;
    let text = res.text().await?;

    let result = serde_json::from_str::<shared::login::LoginSuccess>(&text);
    if let Ok(s) = result {
        Ok(s.auth)
    } else {
        let error_result = serde_json::from_str::<shared::login::LoginError>(&text);
        if let Ok(e) = error_result {
            anyhow::bail!("登录失败: {:?}", e);
        } else {
            anyhow::bail!("无法解析登录响应");
        }
    }
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
