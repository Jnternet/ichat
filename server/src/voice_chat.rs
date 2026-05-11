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
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

#[instrument]
pub async fn run() -> anyhow::Result<()> {
    info!("Initializing voice chat server...");

    let server_db_url = std::env::var("SERVER_DATABASE")?;
    info!("Connecting to database: {}", server_db_url);
    let db = Database::connect(server_db_url).await.map_err(|e| {
        error!("Failed to connect to database: {:?}", e);
        e
    })?;
    info!("Database connection established");

    let server_addr = std::env::var("SERVER_VOICE_CHAT_ADDR")?;
    info!("Binding to address: {}", server_addr);
    let listener = TcpListener::bind(server_addr.clone()).await.map_err(|e| {
        error!("Failed to bind to address {}: {:?}", server_addr, e);
        e
    })?;

    let tls_acceptor = get_acceptor().await.map_err(|e| {
        error!("Failed to create TLS acceptor: {:?}", e);
        e
    })?;
    info!("TLS acceptor created");

    let online_groups = OnlineGroups::default();
    info!("Voice chat server ready, waiting for connections...");

    loop {
        let (stream, addr) = listener.accept().await?;
        info!("Client connected: {}", addr);

        let tls_stream = match tls_acceptor.accept(stream).await {
            Ok(s) => TlsStream::from(s),
            Err(e) => {
                warn!("TLS handshake failed for {}: {:?}", addr, e);
                continue;
            }
        };

        let db_ = db.clone();
        let online_groups_ = online_groups.clone();
        tokio::spawn(async move {
            let r = handle_client(db_, tls_stream, online_groups_).await;
            if r.is_err() {
                error!("Voice chat client handler error: {:?}", r);
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

#[instrument(skip(db, online_groups))]
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

    debug!(
        "Voice chat auth received for account: {}, group: {}",
        auth.account_id(),
        gid.0
    );

    if !crate::auth::auth(&db, &auth).await {
        warn!(
            "Voice chat authentication failed for account: {}",
            auth.account_id()
        );
        bail!("no auth to chat")
    }
    info!(
        "Voice chat authentication successful for account: {}, group: {}",
        auth.account_id(),
        gid.0
    );

    let (s, r) = {
        let mut g = online_groups.hm.write().expect("read lock poisoned");
        let vg = g.entry(gid).or_insert_with(|| {
            let (s, r) = broadcast::channel(1000);
            VoiceGroup { sender: s, _r: r }
        });
        (vg.sender.clone(), vg.sender.subscribe())
    };
    debug!("Joined voice group: {}", gid.0);

    tokio::select! {
        r = handle_rh(helper, s, auth.account_id()) => {
            if r.is_err() {
                warn!("Voice chat read handler error: {:?}", r);
            }
        }
        r = handle_wh(wh, r) => {
            if r.is_err() {
                warn!("Voice chat write handler error: {:?}", r);
            }
        }
    };
    info!(
        "Voice chat client handler exiting for account: {}",
        auth.account_id()
    );
    Ok(())
}

#[instrument(skip(s, rh))]
async fn handle_rh(
    mut rh: ReadHelper<ReadHalf<TlsStream<TcpStream>>>,
    s: broadcast::Sender<S2C_VC_Msg>,
    sender_id: Uuid,
) -> anyhow::Result<()> {
    info!(
        "Starting voice chat read handler for account: {}",
        sender_id
    );
    let mut buf = BytesMut::zeroed(32768);
    while let Some(u) = rh.next_item(&mut buf).await {
        let ar = rkyv::access::<ArchivedC2S_VC_Msg, rancor::Error>(&buf[..u])?;
        let c2s = rkyv::deserialize::<C2S_VC_Msg, rancor::Error>(ar)?;
        let s2c = S2C_VC_Msg {
            sender_id,
            voice_data: c2s.voice_data,
        };
        let l = s2c.voice_data.len();
        let _ = s.send(s2c);
        debug!("Voice data received from {}, length: {}", sender_id, l);
    }
    info!("Voice chat read handler exiting for account: {}", sender_id);
    bail!("no more vc msg")
}

#[instrument]
async fn handle_wh(
    mut wh: WriteHalf<TlsStream<TcpStream>>,
    mut r: broadcast::Receiver<S2C_VC_Msg>,
) -> anyhow::Result<()> {
    info!("Starting voice chat write handler");
    while let Ok(s2c) = r.recv().await {
        let b = rkyv::to_bytes::<rancor::Error>(&s2c)?;
        wh.write_u64(b.len() as u64).await?;
        wh.write_all(&b).await?;
        wh.flush().await?;
        debug!("Voice data sent, length: {}", b.len());
    }
    info!("Voice chat write handler exiting");
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
