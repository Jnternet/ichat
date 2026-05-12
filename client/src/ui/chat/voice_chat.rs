use bytes::BytesMut;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rkyv::rancor;
use rustls::crypto::aws_lc_rs;
use shared::auth::Auth;
use shared::group::GroupId;
use shared::tcp_helper::ReadHelper;
use shared::voice_chat::{C2S_VC_Msg, S2C_VC_Msg, VoiceGroupAuth};
use shared::{voice_chat::ArchivedS2C_VC_Msg, *};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio_rustls::{TlsConnector, TlsStream};
use uuid::Uuid;

#[allow(dead_code)]
struct VoiceStreamData {
    input_stream: cpal::Stream,
    output_stream: cpal::Stream,
}

struct VoiceAudioState {
    sender_hm: HashMap<Uuid, Sender<Vec<f32>>>,
    receivers: Vec<Receiver<Vec<f32>>>,
    control_tx: Option<Sender<()>>,
    is_running: bool,
}

impl VoiceAudioState {
    fn new() -> Self {
        VoiceAudioState {
            sender_hm: HashMap::new(),
            receivers: Vec::new(),
            control_tx: None,
            is_running: true,
        }
    }

    fn stop(&mut self) {
        self.is_running = false;
        if let Some(tx) = self.control_tx.take() {
            let _ = tx.try_send(());
        }
        self.sender_hm.clear();
        self.receivers.clear();
    }

    fn add_sender(&mut self, id: Uuid) -> Sender<Vec<f32>> {
        let (s, r) = channel(4096);
        self.receivers.push(r);
        self.sender_hm.insert(id, s.clone());
        s
    }

    fn get_sender(&mut self, id: Uuid) -> Sender<Vec<f32>> {
        self.sender_hm
            .get(&id)
            .cloned()
            .unwrap_or_else(|| self.add_sender(id))
    }

    fn collect_audio(&mut self) -> Vec<Vec<f32>> {
        let mut result = Vec::new();
        self.receivers.retain_mut(|r| {
            while let Ok(vd) = r.try_recv() {
                result.push(vd);
            }
            !r.is_closed()
        });
        result
    }
}

lazy_static::lazy_static! {
    static ref VOICE_STREAM: Mutex<Option<VoiceStreamData>> = Mutex::new(None);
    static ref AUDIO_STATE: Mutex<VoiceAudioState> = Mutex::new(VoiceAudioState::new());
}

pub async fn start_voice_chat(auth: Auth, gid: GroupId) -> anyhow::Result<()> {
    let _ = aws_lc_rs::default_provider().install_default();

    let server_addr = match std::env::var("SERVER_VOICE_CHAT_ADDR") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: SERVER_VOICE_CHAT_ADDR not set: {}", e);
            return Err(anyhow::anyhow!("SERVER_VOICE_CHAT_ADDR not set"));
        }
    };

    let server_name = match std::env::var("SERVER_NAME") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: SERVER_NAME not set: {}", e);
            return Err(anyhow::anyhow!("SERVER_NAME not set"));
        }
    };

    let connector = get_connector();
    let tls_stream = match get_tls_stream(&connector, &server_addr, &server_name).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error connecting to voice chat server: {}", e);
            return Err(e);
        }
    };

    let vga = VoiceGroupAuth { auth, gid: gid.0 };
    let b = rkyv::to_bytes::<rancor::Error>(&vga)?;

    let mut tls_stream = tls_stream;
    tls_stream.write_u64(b.len() as u64).await?;
    tls_stream.write_all(&b).await?;
    tls_stream.flush().await?;

    let (read_half, write_half) = tokio::io::split(tls_stream);

    {
        let mut state = AUDIO_STATE.lock().unwrap();
        state.stop();
        *state = VoiceAudioState::new();
    }

    let input_stream = match start_input_stream(write_half).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error starting input stream: {}", e);
            return Err(e);
        }
    };

    let output_stream = match start_output_stream(read_half).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error starting output stream: {}", e);
            return Err(e);
        }
    };

    *VOICE_STREAM.lock().unwrap() = Some(VoiceStreamData {
        input_stream,
        output_stream,
    });

    Ok(())
}

pub fn stop_voice_chat() {
    eprintln!("Stopping voice chat...");

    {
        let mut state = AUDIO_STATE.lock().unwrap();
        state.stop();
    }

    *VOICE_STREAM.lock().unwrap() = None;

    eprintln!("Voice chat stopped");
}

fn get_audio_config() -> cpal::StreamConfig {
    cpal::StreamConfig {
        channels: 2,
        sample_rate: 48000,
        buffer_size: cpal::BufferSize::Default,
    }
}

async fn start_output_stream(rh: ReadHalf<TlsStream<TcpStream>>) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();

    let output_device = match host.default_output_device() {
        Some(d) => d,
        None => {
            eprintln!("Error: no default output device");
            return Err(anyhow::anyhow!("no default output device"));
        }
    };

    let config = get_audio_config();
    let state = Arc::new(Mutex::new(VoiceAudioState::new()));

    let (control_tx, mut control_rx) = channel(1);
    {
        let mut s = state.lock().unwrap();
        s.control_tx = Some(control_tx);
    }

    let state_clone = Arc::clone(&state);

    tokio::spawn(async move {
        let mut rh = ReadHelper::new(rh);
        let mut buf = BytesMut::zeroed(32768);

        eprintln!("Starting voice chat read loop...");

        loop {
            tokio::select! {
                result = rh.next_item(&mut buf) => {
                    match result {
                        Some(u) => {
                            eprintln!("Received voice data: {} bytes", u);

                            let ar = match rkyv::access::<ArchivedS2C_VC_Msg, rancor::Error>(&buf[..u]) {
                                Ok(a) => a,
                                Err(e) => {
                                    eprintln!("Parse error: {}", e);
                                    continue;
                                }
                            };

                            let s2c = match rkyv::deserialize::<S2C_VC_Msg, rancor::Error>(ar) {
                                Ok(s) => s,
                                Err(e) => {
                                    eprintln!("Deserialize error: {}", e);
                                    continue;
                                }
                            };

                            let mut state = state_clone.lock().unwrap();
                            if !state.is_running {
                                eprintln!("Audio state not running, stopping read loop");
                                break;
                            }

                            let sender = state.get_sender(s2c.sender_id);
                            if let Err(e) = sender.try_send(s2c.voice_data) {
                                eprintln!("Failed to send audio data: {}", e);
                            }
                        }
                        None => {
                            eprintln!("No more data, exiting read loop");
                            break;
                        }
                    }
                }
                _ = control_rx.recv() => {
                    eprintln!("Received stop signal, exiting read loop");
                    break;
                }
            }
        }

        eprintln!("Voice chat read loop ended");
    });

    let state_clone = Arc::clone(&state);

    let stream = match output_device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut state = state_clone.lock().unwrap();
            if !state.is_running {
                return;
            }

            let audio_data = state.collect_audio();
            for vd in audio_data {
                data.iter_mut().zip(vd).for_each(|(d, s)| {
                    *d = (s * 2.0).tanh();
                });
            }
        },
        move |err| {
            eprintln!("Voice output error: {:?}", err);
        },
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error building output stream: {}", e);
            return Err(anyhow::anyhow!("failed to build output stream: {}", e));
        }
    };

    if let Err(e) = stream.play() {
        eprintln!("Error playing output stream: {}", e);
        return Err(anyhow::anyhow!("failed to play output stream: {}", e));
    }

    Ok(stream)
}

async fn start_input_stream(
    mut wh: WriteHalf<TlsStream<TcpStream>>,
) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();

    let input_device = match host.default_input_device() {
        Some(d) => d,
        None => {
            eprintln!("Error: no default input device");
            return Err(anyhow::anyhow!("no default input device"));
        }
    };

    let config = get_audio_config();
    let (tx, mut rx) = tokio::sync::mpsc::channel(10000);

    tokio::spawn(async move {
        eprintln!("Starting voice chat write loop...");

        while let Some(c2s) = rx.recv().await {
            match rkyv::to_bytes::<rancor::Error>(&c2s) {
                Ok(b) => {
                    let _ = wh.write_u64(b.len() as u64).await;
                    let _ = wh.write_all(&b).await;
                    let _ = wh.flush().await;
                }
                Err(e) => {
                    eprintln!("Failed to serialize voice data: {}", e);
                }
            }
        }

        eprintln!("Voice chat write loop ended");
    });

    let stream = match input_device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let c2s = C2S_VC_Msg {
                voice_data: data.to_vec(),
            };
            let _ = tx.try_send(c2s);
        },
        move |err| {
            eprintln!("Voice input error: {:?}", err);
        },
        None,
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error building input stream: {}", e);
            return Err(anyhow::anyhow!("failed to build input stream: {}", e));
        }
    };

    if let Err(e) = stream.play() {
        eprintln!("Error playing input stream: {}", e);
        return Err(anyhow::anyhow!("failed to play input stream: {}", e));
    }

    Ok(stream)
}

pub async fn get_tls_stream(
    connector: &TlsConnector,
    server_addr: impl AsRef<str>,
    server_name: impl AsRef<str>,
) -> anyhow::Result<TlsStream<tokio::net::TcpStream>> {
    let server_addr = server_addr.as_ref();
    let server_name = server_name.as_ref().to_owned();

    let tcp = match tokio::net::TcpStream::connect(&server_addr).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("TCP connection failed: {}", e);
            return Err(e.into());
        }
    };

    let stream = match connector.connect(server_name.try_into()?, tcp).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("TLS handshake failed: {}", e);
            return Err(e.into());
        }
    };

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
