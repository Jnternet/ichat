use anyhow::{Context, bail};
use bytes::BytesMut;
use rkyv::rancor;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use sea_orm::{Database, DatabaseConnection};
use shared::group::GroupId;
use shared::rkyv;
use shared::tcp_helper::ReadHelper;
use shared::voice_chat::{
    ArchivedC2S_VC_Msg, ArchivedVoiceGroupAuth, C2S_VC_Msg, S2C_VC_Msg, VoiceGroupAuth,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_rustls::{TlsAcceptor, TlsStream};
use uuid::Uuid;

pub async fn run() -> anyhow::Result<()> {
    //准备数据库
    let server_db_url = std::env::var("SERVER_DATABASE")?;
    let db = Database::connect(server_db_url).await?;

    let server_addr = std::env::var("SERVER_VOICE_CHAT_ADDR")?;
    let listener = TcpListener::bind(server_addr).await?;

    let tls_acceptor = get_acceptor().await?;

    let online_groups = OnlineGroups::default();

    loop {
        let (stream, addr) = listener.accept().await?;
        eprintln!("客户端连接: {}", addr);
        let tls_stream = tls_acceptor.accept(stream).await?;
        let tls_stream = TlsStream::from(tls_stream);

        let db_ = db.clone();
        let online_groups_ = online_groups.clone();
        tokio::spawn(async move {
            let r = handle_client(db_, tls_stream, online_groups_).await;
            if r.is_err() {
                dbg!(&r);
            }
        });
    }
}

struct VoiceGroup {
    sender: broadcast::Sender<S2C_VC_Msg>,
    _r: broadcast::Receiver<S2C_VC_Msg>,
}

#[derive(Default, Clone)]
struct OnlineGroups {
    hm: Arc<RwLock<HashMap<GroupId, VoiceGroup>>>,
}

async fn handle_client(
    db: DatabaseConnection,
    tls_stream: TlsStream<TcpStream>,
    online_groups: OnlineGroups,
) -> anyhow::Result<()> {
    let (rh, wh) = tokio::io::split(tls_stream);
    let mut helper = ReadHelper::new(rh);
    let mut buf = BytesMut::zeroed(4096);
    let n = helper.next_item(&mut buf).await.context("no auth given")?;
    let vga = rkyv::access::<ArchivedVoiceGroupAuth, rancor::Error>(&buf[..n])
        .context("cannot access auth")?;
    let vga = rkyv::deserialize::<VoiceGroupAuth, rancor::Error>(vga)?;
    let auth = vga.auth;
    let gid = vga.gid;
    let gid = GroupId(gid);
    if !crate::auth::auth(&db, &auth).await {
        bail!("no auth to chat")
    }

    let (s, r) = {
        let mut g = online_groups.hm.write().expect("read lock poisoned");
        let vg = g.entry(gid).or_insert_with(|| {
            let (s, r) = broadcast::channel(100);
            VoiceGroup { sender: s, _r: r }
        });
        (vg.sender.clone(), vg.sender.subscribe())
    };

    tokio::select! {
        r = handle_rh(helper, s, auth.account_id()) => {
                dbg!(&r);
            }
        r = handle_wh(wh,r) => {
                dbg!(&r);
            }
    };
    println!("handle_client exit");
    Ok(())
}

async fn handle_rh(
    mut rh: ReadHelper<ReadHalf<TlsStream<TcpStream>>>,
    s: broadcast::Sender<S2C_VC_Msg>,
    sender_id: Uuid,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::zeroed(4096);
    while let Some(u) = rh.next_item(&mut buf).await {
        let ar = rkyv::access::<ArchivedC2S_VC_Msg, rancor::Error>(&buf[..u])?;
        let c2s = rkyv::deserialize::<C2S_VC_Msg, rancor::Error>(ar)?;
        let s2c = S2C_VC_Msg {
            sender_id,
            voice_data: c2s.voice_data,
        };
        s.send(s2c)?;
    }
    bail!("no more vc msg")
}

async fn handle_wh(
    mut wh: WriteHalf<TlsStream<TcpStream>>,
    mut r: broadcast::Receiver<S2C_VC_Msg>,
) -> anyhow::Result<()> {
    while let Ok(s2c) = r.recv().await {
        let b = rkyv::to_bytes::<rancor::Error>(&s2c)?;
        wh.write_u64(b.len() as u64).await?;
        wh.write_all(&b).await?;
        wh.flush().await?
    }
    bail!("cannot send to client")
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
