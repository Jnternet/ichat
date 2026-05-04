use anyhow::Context;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use rustls::crypto::aws_lc_rs;
use sha2::Digest;
use shared::group::GroupId;
use shared::voice_chat::{C2S_VC_Msg, S2C_VC_Msg, VoiceGroupAuth};
use shared::*;
use std::io::stdin;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;

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

    println!("群组id:");
    let mut gid = String::new();
    stdin().read_line(&mut gid)?;
    gid = gid.trim().to_string();
    let gid = gid.parse::<Uuid>()?;
    let gid = GroupId(gid);

    // 3. 建立与服务端的 TLS 连接
    let server_addr = std::env::var("SERVER_VOICE_CHAT_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    let connector = get_connector();
    let mut tls_stream = get_tls_stream(&connector, &server_addr, &server_name).await?;

    let vga = VoiceGroupAuth { auth, gid };
    // 4. 发送 Auth 信息进行认证
    let auth_json = serde_json::to_vec(&vga)?;
    tls_stream.write_all(&auth_json).await?;
    tls_stream.flush().await?;

    // 5. 分离读写流，分别处理消息的发送和接收
    let (read_half, write_half) = tokio::io::split(tls_stream);
    tokio::select! {
        r = handle_read(read_half) => {
            dbg!(r)?;
        },

        r = handle_write(write_half) => {
            dbg!(r)?;
        }
    }

    tokio::signal::ctrl_c().await?;
    Ok(())
}

async fn handle_read(mut rh: ReadHalf<TlsStream<TcpStream>>) -> anyhow::Result<()> {
    let rb = ringbuf::HeapRb::<f32>::new(8192);
    let (mut rp, mut rc) = rb.split();

    let host = cpal::default_host();
    let output = host.default_output_device().expect("no output device");
    let supported_stream_config = output.default_output_config()?;
    let config = supported_stream_config.config();

    let stream = output.build_output_stream(
        &config,
        move |data: &mut [f32], _x| {
            data.iter_mut()
                .zip(rc.pop_iter())
                .for_each(|(data, sample)| {
                    *data = sample;
                });
        },
        move |error| {
            dbg!(error);
        },
        None,
    )?;

    stream.play()?;

    let mut buf = bytes::BytesMut::with_capacity(8192);
    loop {
        let u = rh.read_u64().await?;
        buf.resize(u as usize, 0);
        rh.read_exact(&mut buf).await?;
        let msg = serde_json::from_slice::<S2C_VC_Msg>(&buf)?;
        buf.clear();
        let vd = msg.voice_data;
        let s = u8_to_f32_slice(&vd).unwrap();
        rp.push_slice(s);
    }
}
fn u8_to_f32_slice(slice: &[u8]) -> Option<&[f32]> {
    bytemuck::try_cast_slice(slice).ok()
}
fn f32_to_u8_slice(slice: &[f32]) -> &[u8] {
    bytemuck::cast_slice(slice)
}

async fn handle_write(mut wh: WriteHalf<TlsStream<TcpStream>>) -> anyhow::Result<()> {
    let rb = ringbuf::HeapRb::<f32>::new(40960);
    let (mut rp, mut rc) = rb.split();

    let host = cpal::default_host();

    let input = host.default_input_device().context("未找到默认输入设备")?;
    println!(
        "使用输入设备: {}",
        input.description().context("无法获取设备描述")?
    );

    let s_config = input.default_input_config()?;
    let config = s_config.config();

    println!("\n选择的录音配置:");
    println!("  - 声道数: {}", config.channels);
    println!("  - 采样率: {}", config.sample_rate);
    println!("  - 采样格式: {:?}", s_config.sample_format());
    println!("  - 缓冲区大小: {:?}", config.buffer_size);

    // 根据采样格式创建录音回调，确保数据格式与WAV文件头一致
    let stream = match s_config.sample_format() {
        // 浮点格式转换为16位整数
        SampleFormat::F32 => input.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                //用于调整音量(默认1.5倍音量)
                // let data: Vec<f32> = data.iter().map(|f| (f * 1.5).clamp(-1.0, 1.0)).collect();
                rp.push_slice(data);
            },
            move |err| {
                eprintln!("录音错误: {:?}", err);
            },
            None,
        )?,
        _ => {
            anyhow::bail!("不支持的采样格式: {:?}", s_config.sample_format());
        }
    };

    // 启动录音(新线程
    stream.play()?;
    println!("开始录音...");

    let mut buf = [0f32; 8192];
    loop {
        if rc.occupied_len() == 0 {
            continue;
        }
        let u = rc.pop_slice(&mut buf);
        let s = f32_to_u8_slice(&buf[..u]);
        let b = bytes::Bytes::copy_from_slice(s);
        let c2s = C2S_VC_Msg { voice_data: b };

        let json = serde_json::to_vec(&c2s)?;
        let len = json.len() as u64;
        wh.write_u64(len).await?;
        wh.flush().await?;
        wh.write_all(&json).await?;
        wh.flush().await?;
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
    let server_addr = std::env::var("SERVER_HTTPS_ADDR")?;
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
