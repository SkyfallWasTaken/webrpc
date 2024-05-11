use std::sync::Arc;
use std::time::SystemTime;

use color_eyre::{eyre::WrapErr, Result};

use discord_sdk::{activity, AppId};
use futures_util::{future, stream::SplitSink, SinkExt, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::{tungstenite::Message as TungsteniteMessage, WebSocketStream};

use crate::client;
use crate::services::Service;

pub async fn start(client: Arc<Mutex<client::Client>>) -> Result<()> {
    let addr = "0.0.0.0:3000";
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.wrap_err("Failed to bind")?;
    tracing::info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let client = Arc::clone(&client);
        tokio::spawn(async move {
            let lock = client.lock().await;
            if let Err(e) = accept_connection(stream, &lock).await {
                tracing::error!(error = ?e, "error accepting connection");
            }
        });
    }

    Ok(())
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
        id: u16,
    },

    Clear {
        id: u16,
    },

    Ack {
        id: u16,
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
    let (write, read) = ws_stream.split();
    let write = Arc::new(Mutex::new(write)); // Wrap the write handle in an Arc<Mutex<>>

    (*(*write.clone()).lock().await)
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
        .try_for_each_concurrent(None, |msg| {
            let text = msg.to_text().unwrap(); // We filtered out non-Text messages
            let msg: Message = serde_json::from_str(text).unwrap();
            tracing::info!(message = ?msg, "received message");

            let write = Arc::clone(&write); // Clone the Arc, not the SplitSink

            async move {
                let mut write = write.lock().await; // Lock the Mutex to get a mutable reference to the SplitSink
                match msg {
                    Message::Update { service, id } => {
                        update_presence(client, service, &mut write, id).await;
                    }
                    Message::Clear { id } => {
                        let clear_result = client.discord.clear_activity().await;
                        match clear_result {
                            Ok(_) => tracing::info!("activity cleared"),
                            Err(e) => tracing::error!("failed to clear activity: {}", e),
                        }

                        write
                            .send(TungsteniteMessage::Text(
                                serde_json::to_string(&Message::Ack { id }).unwrap(),
                            ))
                            .await
                            .unwrap();
                    }
                    _ => unreachable!(),
                }
                Ok(())
            }
        })
        .await?;

    Ok(())
}

async fn update_presence(
    client: &client::Client,
    service: Service,
    write: &mut SplitSink<WebSocketStream<TcpStream>, TungsteniteMessage>,
    msg_id: u16,
) {
    tracing::info!(service = ?service, "received service update");
    match service {
        Service::YouTubeMusic(info) => {
            let mut assets = activity::Assets::default().large(
                "youtubemusic".to_owned(),
                Some(format!("{} - {}", info.artist, info.song)),
            );
            if info.paused {
                assets = assets.small("paused".to_owned(), Some("Paused".to_owned()));
            }
            let rp = activity::ActivityBuilder::default()
                .details(info.song)
                .state(info.artist)
                .assets(assets)
                .start_timestamp(SystemTime::now());

            tracing::info!(
                "updated activity: {:?}",
                client.discord.update_activity(rp).await
            );
        }
    }

    write
        .send(TungsteniteMessage::Text(
            serde_json::to_string(&Message::Ack { id: msg_id }).unwrap(),
        ))
        .await
        .unwrap();
}
