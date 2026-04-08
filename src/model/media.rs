#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArtworkSlot {
    Primary,
    Secondary,
}

impl ArtworkSlot {
    pub(crate) fn other(self) -> Self {
        match self {
            Self::Primary => Self::Secondary,
            Self::Secondary => Self::Primary,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaState {
    pub(crate) status: PlaybackStatus,
    pub(crate) art_url: Option<String>,
    pub(crate) metadata: TrackMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TrackMetadata {
    pub(crate) artist: String,
    pub(crate) title: String,
    pub(crate) album: String,
    pub(crate) track_number: String,
    pub(crate) length: String,
    pub(crate) length_microseconds: Option<u64>,
    pub(crate) position: String,
    pub(crate) position_microseconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaybackStatus {
    Playing,
    Paused,
    NotPlaying,
}

impl PlaybackStatus {
    pub(crate) fn should_show_artwork(self, show_paused: bool) -> bool {
        self == Self::Playing || (show_paused && self == Self::Paused)
    }

    pub(crate) fn auto_select_rank(self) -> u8 {
        match self {
            Self::Playing => 2,
            Self::Paused => 1,
            Self::NotPlaying => 0,
        }
    }
}
