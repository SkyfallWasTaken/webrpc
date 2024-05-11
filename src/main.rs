use std::sync::Arc;
use tokio::sync::Mutex;

use color_eyre::Result;

use discord_sdk::Subscriptions;

mod client;
mod server;
mod services;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::INFO)
        .init();

    let client = client::Client::from_subscriptions(Subscriptions::ACTIVITY).await?;
    tracing::debug!("Created Discord client!");

    let mut activity_events = client.wheel.activity();

    tokio::task::spawn(async move {
        while let Ok(ae) = activity_events.0.recv().await {
            tracing::info!(event = ?ae, "received activity event");
        }
    });

    server::start(Arc::new(Mutex::new(client))).await?;

    Ok(())
}
