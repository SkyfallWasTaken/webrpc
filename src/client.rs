use color_eyre::{eyre::Context, Result};
use discord_sdk::{
    user::User,
    wheel::{UserState, Wheel},
    AppId, Discord, DiscordApp, Subscriptions,
};

/// Application identifier for "Andy's Test App" used in the Discord SDK's
/// examples.
pub const APP_ID: AppId = 1238806174078472223;

pub struct Client {
    pub discord: Discord,
    pub user: User,
    pub wheel: Wheel,
}

pub async fn make_client(subs: Subscriptions) -> Result<Client> {
    let (wheel, handler) = Wheel::new(Box::new(|err| {
        tracing::error!(error = ?err, "encountered an error");
    }));

    let mut user = wheel.user();

    let discord = Discord::new(DiscordApp::PlainId(APP_ID), subs, Box::new(handler))
        .wrap_err("unable to create discord client")?;

    tracing::info!("waiting for handshake...");
    user.0.changed().await.unwrap();

    let user = match &*user.0.borrow() {
        UserState::Connected(user) => user.clone(),
        UserState::Disconnected(err) => {
            tracing::error!(error = ?err, "failed to connect to Discord");
            panic!() // FIXME: awful!
        }
    };

    tracing::info!("connected to Discord, local user is {:#?}", user);

    Ok(Client {
        discord,
        user,
        wheel,
    })
}
