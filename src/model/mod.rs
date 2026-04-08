mod config;
mod media;
mod metadata;

pub(crate) use config::{ArtworkFit, AxisPlacement, Config, Placement, ShellLayer, Transition};
pub(crate) use media::{ArtworkSlot, MediaState, PlaybackStatus, TrackMetadata};
pub(crate) use metadata::{
    MetadataConfig, MetadataSection, MetadataSectionConfig, MetadataStyleConfig, RevealDirection,
    SectionAlign, TextAnimationConfig, TextAnimationMode, TruncateMode,
};
