use anyhow::Context;
use bytes::BytesMut;
use ringbuf::traits::{Consumer, Producer, Split};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rkyv::rancor;
use rustls::crypto::aws_lc_rs;
use sha2::Digest;
use shared::{
    tcp_helper::ReadHelper,
    voice_chat::{C2S_VC_Msg, S2C_VC_Msg, VoiceGroupAuth},
};
use shared::{voice_chat::ArchivedS2C_VC_Msg, *};
use std::sync::Arc;
use std::{collections::HashMap, io::stdin};
use tokio::{
    io::{AsyncWriteExt, ReadHalf, WriteHalf},
    sync::mpsc::{Sender, channel},
};
use tokio::{net::TcpStream, sync::mpsc::Receiver};
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

    // 3. 建立与服务端的 TLS 连接
    let server_addr = std::env::var("SERVER_VOICE_CHAT_ADDR")?;
    let server_name = std::env::var("SERVER_NAME")?;
    let connector = get_connector();
    let mut tls_stream = get_tls_stream(&connector, &server_addr, &server_name).await?;

    let vga = VoiceGroupAuth { auth, gid };
    // 4. 发送 Auth 信息进行认证
    let b = rkyv::to_bytes::<rancor::Error>(&vga)?;
    tls_stream.write_u64(b.len() as u64).await?;
    tls_stream.write_all(&b).await?;
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

fn get_audio_config() -> cpal::StreamConfig {
    cpal::StreamConfig {
        channels: 2,
        sample_rate: 48000,
        buffer_size: cpal::BufferSize::Default,
    }
}

#[derive(Default, Debug)]
struct HV {
    sender_hm: HashMap<Uuid, Sender<Vec<f32>>>,
    rec_v: Vec<Receiver<Vec<f32>>>,
}

impl HV {
    fn add(&mut self, id: Uuid) -> anyhow::Result<Sender<Vec<f32>>> {
        let (s, r) = channel(4096);

        self.sender_hm.insert(id, s.clone()).context("重复插入")?;
        self.rec_v.push(r);

        Ok(s)
    }

    fn send_vd(&mut self, msg: S2C_VC_Msg) -> anyhow::Result<()> {
        let id = msg.sender_id;
        let vd = msg.voice_data;

        let o = self.sender_hm.get(&id);
        let s = match o {
            Some(s) => s.clone(),
            None => self.add(id).unwrap(),
        };
        s.try_send(vd)?;

        Ok(())
    }
    async fn one_sound(&mut self) -> Vec<Vec<f32>> {
        let mut ans = Vec::new();

        for r in &mut self.rec_v {
            let Some(vd) = r.recv().await else {
                continue;
            };
            ans.push(vd);
        }
        ans
    }
    fn split(self) -> (HVS, HVR) {
        let (s, r) = channel(100);
        let hvs = HVS {
            sender: s,
            hm: self.sender_hm,
        };
        let hvr = HVR {
            reciver: r,
            v: self.rec_v,
        };
        (hvs, hvr)
    }
}

struct HVS {
    sender: Sender<Receiver<Vec<f32>>>,
    hm: HashMap<Uuid, Sender<Vec<f32>>>,
}
impl HVS {
    fn add(&mut self, id: Uuid) -> anyhow::Result<Sender<Vec<f32>>> {
        let (s, r) = channel(4096);

        self.sender.try_send(r)?;
        self.hm.insert(id, s.clone()).context("重复插入")?;
        Ok(s)
    }
    fn send(&mut self, msg: S2C_VC_Msg) -> anyhow::Result<()> {
        let id = msg.sender_id;
        let vd = msg.voice_data;

        let o = self.hm.get(&id);
        let s = match o {
            Some(s) => s.clone(),
            None => self.add(id).unwrap(),
        };
        s.try_send(vd)?;

        Ok(())
    }
}
struct HVR {
    reciver: Receiver<Receiver<Vec<f32>>>,
    v: Vec<Receiver<Vec<f32>>>,
}

impl HVR {
    fn update(&mut self) {
        let Ok(r) = self.reciver.try_recv() else {
            return;
        };
        self.v.push(r);
    }
    fn one_sound(&mut self) -> Vec<Vec<f32>> {
        self.update();
        let mut ans = Vec::new();
        for r in &mut self.v {
            let Ok(vd) = r.try_recv() else {
                continue;
            };
            ans.push(vd);
        }
        ans
    }
}

async fn handle_read(rh: ReadHalf<TlsStream<TcpStream>>) -> anyhow::Result<()> {
    let mut rh = ReadHelper::new(rh);

    let rb = ringbuf::HeapRb::<f32>::new(32768);
    let (mut rp, mut rc) = rb.split();

    let mut hv = HV::default();
    let (mut hvs, mut hvr) = hv.split();

    let host = cpal::default_host();
    let output = host.default_output_device().unwrap();

    let config = get_audio_config();
    let stream = output
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let v = hvr.one_sound();
                for vd in v {
                    data.iter_mut().zip(vd).for_each(|(d, s)| {
                        *d = (s * 2.0).tanh();
                    });
                }
            },
            move |err| {
                dbg!(&err);
            },
            None,
        )
        .unwrap();

    stream.play().unwrap();

    let mut buf = BytesMut::zeroed(32768);
    while let Some(u) = rh.next_item(&mut buf).await {
        let ar = rkyv::access::<ArchivedS2C_VC_Msg, rancor::Error>(&buf[..u])
            .context("cannot parse S2C_VC_Msg")
            .unwrap();
        let s2c = rkyv::deserialize::<S2C_VC_Msg, rancor::Error>(ar)
            .context("cannot deserialize")
            .unwrap();
        hvs.send(s2c)?
    }

    Ok(())
}
async fn handle_write(mut wh: WriteHalf<TlsStream<TcpStream>>) -> anyhow::Result<()> {
    let (s, mut r) = tokio::sync::mpsc::channel(10000);

    let host = cpal::default_host();
    let input = host.default_input_device().unwrap();
    let config = get_audio_config();

    let stream = input
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let c2s = C2S_VC_Msg {
                    voice_data: data.to_vec(),
                };
                let _ = s.try_send(c2s);
            },
            move |err| {
                dbg!(&err);
            },
            None,
        )
        .unwrap();

    stream.play().unwrap();

    while let Some(c2s) = r.recv().await {
        let b = rkyv::to_bytes::<rancor::Error>(&c2s).context("cannot serde to bytes")?;
        wh.write_u64(b.len() as u64).await?;
        wh.write_all(&b).await?;
        wh.flush().await?;
    }

    Ok(())
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
