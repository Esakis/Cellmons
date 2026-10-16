use crate::{Measurements, fixture::Player};
use anyhow::{Context as _, ensure};
use futures_util::{SinkExt as _, StreamExt as _};
use protocol::{
    Message as _,
    game::v1::{self as pb, client_message::Body as Request, server_message::Body},
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{Message, client::IntoClientRequest as _},
};

type Reply = oneshot::Sender<anyhow::Result<Body>>;
pub struct Client {
    tx: mpsc::Sender<(Request, Reply)>,
    task: tokio::task::JoinHandle<()>,
    pub resume: String,
}
impl Drop for Client {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Client {
    pub async fn connect(
        addr: SocketAddr,
        player: &Player,
        resume: Option<String>,
        metrics: Arc<Measurements>,
    ) -> anyhow::Result<Self> {
        let start = Instant::now();
        let mut request = format!("ws://{addr}/ws").into_client_request()?;
        request
            .headers_mut()
            .insert("origin", "http://localhost:5173".parse()?);
        request
            .headers_mut()
            .insert("cookie", format!("game_session={}", player.token).parse()?);
        let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .read_buffer_size(16 * 1024)
            .write_buffer_size(4096);
        let (mut socket, _) = tokio::time::timeout(
            Duration::from_secs(30),
            connect_async_with_config(request, Some(config), true),
        )
        .await??;
        let hello = resume
            .map(|resume_token| Request::Resume(pb::Resume { resume_token }))
            .unwrap_or_else(|| {
                Request::Hello(pb::Hello {
                    protocol_major: 1,
                    content_version: engine_content::CONTENT_VERSION.to_vec(),
                    locale: "en".into(),
                    client_kind: "web".into(),
                    build: "load-fixture".into(),
                    ..Default::default()
                })
            });
        socket
            .send(Message::Binary(
                pb::ClientMessage {
                    seq: 1,
                    body: Some(hello),
                }
                .encode_to_vec()
                .into(),
            ))
            .await?;
        let welcome = tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(message) = socket.next().await {
                match message? {
                    Message::Binary(bytes) => {
                        let frame = pb::ServerMessage::decode(bytes)?;
                        if frame.reply_to == 1 {
                            return Ok::<_, anyhow::Error>(frame.body);
                        }
                    }
                    Message::Ping(_) => socket.flush().await?,
                    Message::Close(_) => anyhow::bail!("connection closed before welcome"),
                    _ => {}
                }
            }
            anyhow::bail!("connection ended before welcome")
        })
        .await??;
        let Some(Body::Welcome(welcome)) = welcome else {
            anyhow::bail!("expected welcome")
        };
        metrics.record("connection", start);
        let (tx, mut requests) = mpsc::channel::<(Request, Reply)>(16);
        // Observe a fixed, distributed 1% sample; all clients still decode and drain every frame.
        // A shared histogram mutex on every fanout delivery would benchmark the generator lock.
        let sample_chat = player
            .login
            .strip_prefix("Load")
            .and_then(|index| index.parse::<usize>().ok())
            .is_some_and(|index| index % 100 == 0);
        let task = tokio::spawn(async move {
            let mut seq = 1u32;
            let mut pending = HashMap::<u32, Reply>::new();
            loop {
                tokio::select! {
                    request=requests.recv()=>{
                        let Some((body,reply))=request else {break;};
                        seq+=1;pending.insert(seq,reply);
                        if socket.send(Message::Binary(pb::ClientMessage{seq,body:Some(body)}.encode_to_vec().into())).await.is_err(){break;}
                    },
                    message=socket.next()=>{
                        let Some(Ok(message))=message else {break;};
                        match message {
                            Message::Binary(bytes)=>{
                                let Ok(frame)=pb::ServerMessage::decode(bytes) else {break;};
                            if let Some(Body::ChatMessage(ref chat))=frame.body && sample_chat
                                    && let Some(sent)=chat.text.strip_prefix("load:").and_then(|s|s.parse::<u128>().ok()) {
                                        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_micros();
                                        metrics.value("chat_delivery",now.saturating_sub(sent).min(u128::from(u64::MAX)) as u64);
                                }
                                if let Some(reply)=pending.remove(&frame.reply_to) {
                                    let body=frame.body.context("empty protocol response");let _=reply.send(body);
                                }
                            },
                            Message::Ping(_)=>{if socket.flush().await.is_err(){break;}},
                            Message::Close(frame)=>{
                                metrics.value("transport_close_code",frame.map(|frame|u64::from(u16::from(frame.code))).unwrap_or(0));
                                break;
                            },_=>{},
                        }
                    }
                }
            }
            for (_, reply) in pending {
                let _ = reply.send(Err(anyhow::anyhow!("transport closed")));
            }
        });
        Ok(Self {
            tx,
            task,
            resume: welcome.resume_token,
        })
    }
    pub async fn call(&self, body: Request) -> anyhow::Result<Body> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send((body, reply))
            .await
            .context("load client closed")?;
        let body = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .context("protocol response timed out")??;
        let body = body?;
        if let Body::Error(error) = &body {
            anyhow::bail!("protocol rejected: {}", error.code);
        }
        Ok(body)
    }
    pub async fn join(&self, room: &str) -> anyhow::Result<usize> {
        let Body::RoomSnapshot(snapshot) = self
            .call(Request::RoomJoin(pb::RoomJoin { room: room.into() }))
            .await?
        else {
            anyhow::bail!("expected room snapshot")
        };
        Ok(snapshot.encoded_len())
    }
    pub async fn ping(&self) -> anyhow::Result<()> {
        ensure!(
            matches!(self.call(Request::Ping(pb::Ping {})).await?, Body::Pong(_)),
            "expected pong"
        );
        Ok(())
    }
}
