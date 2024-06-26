use color_eyre::{eyre::Context, Result};
use discord_sdk::{
    user::User,
    wheel::{UserState, Wheel},
    AppId, Discord, DiscordApp, Subscriptions,
};

#[allow(clippy::unreadable_literal)]
pub const APP_ID: AppId = 1238876651492868157;

pub struct Client {
    pub discord: Discord,
    pub user: User,
    pub wheel: Wheel,
}

impl Client {
    pub async fn from_subscriptions(subs: Subscriptions) -> Result<Self> {
        let (wheel, handler) = Wheel::new(Box::new(|err| {
            tracing::error!(error = ?err, "encountered an error");
        }));

        let mut user = wheel.user();

        let discord = Discord::new(DiscordApp::PlainId(APP_ID), subs, Box::new(handler))
            .wrap_err("unable to create discord client")?;

        tracing::info!("waiting for handshake...");
        user.0.changed().await?;

        let user = match &*user.0.borrow() {
            UserState::Connected(user) => user.clone(),
            UserState::Disconnected(err) => {
                tracing::error!(error = ?err, "failed to connect to Discord");
                panic!("failed to connect to Discord"); // FIXME: better error handling
            }
        };

        tracing::info!("connected to Discord, local user is {:#?}", user);

        Ok(Self {
            discord,
            user,
            wheel,
        })
    }
}
