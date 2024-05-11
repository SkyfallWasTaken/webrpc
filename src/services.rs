use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct MusicServiceInfo {
    pub song: String,
    pub artist: String,
    pub paused: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Service {
    YouTubeMusic(MusicServiceInfo),
}
