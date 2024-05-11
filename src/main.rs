use std::sync::Arc;
use std::time::SystemTime;

use color_eyre::{eyre::WrapErr, Result};

use discord_sdk::{activity, AppId, Subscriptions};
use futures_util::{future, SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

mod client;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = client::make_client(Subscriptions::ACTIVITY).await?;
    tracing::debug!("Created Discord client!");

    let mut activity_events = client.wheel.activity();

    tokio::task::spawn(async move {
        while let Ok(ae) = activity_events.0.recv().await {
            tracing::info!(event = ?ae, "received activity event");
        }
    });

    start_server(Arc::new(Mutex::new(client))).await?;

    Ok(())
}

async fn start_server(client: Arc<Mutex<client::Client>>) -> Result<()> {
    let addr = "0.0.0.0:3000";
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.wrap_err("Failed to bind")?;
    tracing::info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let client = Arc::clone(&client);
        tokio::spawn(async move {
            let lock = client.lock().await;
            accept_connection(stream, &*lock).await.unwrap();
        });
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct MusicServiceInfo {
    song: String,
    artist: String,
}

#[derive(Serialize, Deserialize, Debug)]
enum Service {
    YouTubeMusic(MusicServiceInfo),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Message {
    Hello {
        msg: String,
        time: SystemTime,
        app_id: AppId,
    },

    Update {
        service: Service,
    },
}

async fn accept_connection(stream: TcpStream, client: &client::Client) -> Result<()> {
    let addr = stream
        .peer_addr()
        .expect("connected streams should have a peer address");
    tracing::info!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .expect("Error during the websocket handshake occurred");

    tracing::debug!("New WebSocket connection: {}", addr);

    // Firstly, let's say Hello!
    let (mut write, read) = ws_stream.split();
    write
        .send(TungsteniteMessage::Text(
            serde_json::to_string(&Message::Hello {
                msg: "hello from webrpc :D".to_string(),
                time: SystemTime::now(),
                app_id: client::APP_ID,
            })
            .unwrap(),
        ))
        .await
        .expect("Failed to say hello");

    // Now, let's read incoming messages
    read.try_filter(|msg| future::ready(msg.is_text()))
        .try_for_each(|msg| {
            let text = msg.to_text().unwrap(); // We filtered out non-Text messages
            let msg: Message = serde_json::from_str(text).unwrap();
            tracing::info!(message = ?msg, "received message");

            async {
                match msg {
                    Message::Update { service } => {
                        tracing::info!(service = ?service, "received service update");
                        match service {
                            Service::YouTubeMusic(info) => {
                                let rp = activity::ActivityBuilder::default()
                                    .details(info.song)
                                    .state(info.artist)
                                    .start_timestamp(SystemTime::now());

                                tracing::info!(
                                    "updated activity: {:?}",
                                    client.discord.update_activity(rp).await
                                );
                            }
                        }
                    }
                    _ => unreachable!(),
                }
                Ok(())
            }
        })
        .await?;

    Ok(())
}
