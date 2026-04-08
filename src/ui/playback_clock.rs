use std::time::{Duration, Instant};

use crate::model::TrackMetadata;

const CLOCK_REANCHOR_DRIFT_MICROSECONDS: u64 = 400_000;
pub(super) const POSITION_TICK_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TrackIdentity(String);

impl TrackIdentity {
    pub(super) fn from_metadata(art_url: Option<&str>, metadata: &TrackMetadata) -> Self {
        Self(
            [
                metadata.artist.as_str(),
                metadata.title.as_str(),
                metadata.album.as_str(),
                metadata.track_number.as_str(),
                metadata.length.as_str(),
                art_url.unwrap_or_default(),
            ]
            .join("\u{1f}"),
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlaybackClock {
    track: TrackIdentity,
    anchor_position_microseconds: u64,
    anchor_time: Instant,
    length_microseconds: Option<u64>,
}

impl PlaybackClock {
    pub(super) fn track(&self) -> &TrackIdentity {
        &self.track
    }

    pub(super) fn sync(
        existing: Option<Self>,
        track: TrackIdentity,
        anchor_position_microseconds: u64,
        length_microseconds: Option<u64>,
    ) -> Self {
        if let Some(mut existing) = existing
            && existing.track == track
        {
            let drift = existing
                .position_microseconds_now()
                .abs_diff(anchor_position_microseconds);

            if drift <= CLOCK_REANCHOR_DRIFT_MICROSECONDS {
                existing.length_microseconds = length_microseconds;
                return existing;
            }
        }

        Self {
            track,
            anchor_position_microseconds,
            anchor_time: Instant::now(),
            length_microseconds,
        }
    }

    fn position_microseconds_now(&self) -> u64 {
        let elapsed_microseconds =
            u64::try_from(self.anchor_time.elapsed().as_micros()).unwrap_or(u64::MAX);

        self.anchor_position_microseconds
            .saturating_add(elapsed_microseconds)
    }

    pub(super) fn clamped_position_microseconds_now(&self) -> u64 {
        let position = self.position_microseconds_now();
        if let Some(length_microseconds) = self.length_microseconds {
            position.min(length_microseconds)
        } else {
            position
        }
    }
}
