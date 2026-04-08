use gtk::{glib, prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{
    artwork::download_texture,
    metadata::{self, MetadataWidgets},
    model::{ArtworkSlot, Config, MediaState, PlaybackStatus, ShellLayer},
    player::query_player,
    timestamp::{format_timestamp_microseconds, parse_timestamp_microseconds},
    transitions::{clear_artwork, set_artwork_texture},
};

use super::{
    ArtworkLayer,
    layout::sync_window_target,
    playback_clock::{POSITION_TICK_INTERVAL, PlaybackClock, TrackIdentity},
};

const MEDIA_MISS_GRACE: Duration = Duration::from_secs(5);
const MPRIS_EVENT_PUMP_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone)]
pub(super) struct UiRefreshState {
    pub(super) window: gtk::ApplicationWindow,
    pub(super) config: Rc<Config>,
    pub(super) monitor_status: Rc<RefCell<Option<String>>>,
    pub(super) artwork_stack: gtk::Fixed,
    pub(super) primary_artwork: ArtworkLayer,
    pub(super) secondary_artwork: ArtworkLayer,
    pub(super) metadata_widgets: MetadataWidgets,
    pub(super) current_url: Rc<RefCell<Option<String>>>,
    pub(super) active_slot: Rc<RefCell<ArtworkSlot>>,
    pub(super) transition_source: Rc<RefCell<Option<glib::SourceId>>>,
    pub(super) splash_active: Rc<RefCell<bool>>,
    pub(super) media_miss_since: Rc<RefCell<Option<Instant>>>,
    pub(super) last_track_identity: Rc<RefCell<Option<TrackIdentity>>>,
    pub(super) last_media_state: Rc<RefCell<Option<MediaState>>>,
    pub(super) playback_clock: Rc<RefCell<Option<PlaybackClock>>>,
}

pub(super) fn install_refresh_loop(state: UiRefreshState) {
    let initial = state.clone();
    glib::idle_add_local_once(move || {
        initial.refresh();
    });

    let (mpris_event_tx, mpris_event_rx) = std::sync::mpsc::channel::<()>();
    crate::mpris::start_signal_bridge(mpris_event_tx, state.config.poll_seconds);

    let from_signal = state.clone();
    glib::timeout_add_local(MPRIS_EVENT_PUMP_INTERVAL, move || {
        let mut has_event = false;
        while mpris_event_rx.try_recv().is_ok() {
            has_event = true;
        }

        if has_event {
            from_signal.refresh();
        }

        glib::ControlFlow::Continue
    });

    let from_poll = state.clone();
    glib::timeout_add_seconds_local(state.config.poll_seconds, move || {
        from_poll.refresh();
        glib::ControlFlow::Continue
    });

    let position_tick_state = state.clone();
    glib::timeout_add_local(POSITION_TICK_INTERVAL, move || {
        position_tick_state.tick_position_display();
        glib::ControlFlow::Continue
    });
}

impl UiRefreshState {
    fn refresh(&self) {
        sync_window_target(&self.window, &self.config, &self.monitor_status);

        let include_metadata = self.config.metadata.enabled
            && (self.config.metadata.top.enabled || self.config.metadata.left.enabled);

        match query_player(&self.config.player, include_metadata) {
            Some(state) if state.status.should_show_artwork(self.config.show_paused) => {
                *self.media_miss_since.borrow_mut() = None;
                let mut has_any_artwork = self.current_url.borrow().is_some();

                if let Some(art_url) = state.art_url.as_ref() {
                    let needs_reload = self
                        .current_url
                        .borrow()
                        .as_ref()
                        .map(|current| current != art_url)
                        .unwrap_or(true);

                    if needs_reload {
                        match download_texture(art_url, &self.config) {
                            Some(texture) => {
                                set_artwork_texture(
                                    &self.artwork_stack,
                                    &self.primary_artwork,
                                    &self.secondary_artwork,
                                    &self.active_slot,
                                    &self.transition_source,
                                    &self.config,
                                    &texture,
                                );
                                *self.current_url.borrow_mut() = Some(art_url.clone());
                                has_any_artwork = true;
                            }
                            None => {
                                eprintln!(
                                    "covermint: failed to download artwork, keeping previous cover if available"
                                );
                            }
                        }
                    } else {
                        has_any_artwork = true;
                    }
                }

                if !has_any_artwork {
                    self.handle_empty_state();
                    return;
                }

                *self.last_media_state.borrow_mut() = Some(state.clone());
                if include_metadata {
                    self.sync_playback_clock(&state);
                } else {
                    *self.playback_clock.borrow_mut() = None;
                }

                let rendered = metadata::render_metadata(&self.config.metadata, &state.metadata);
                let animate_metadata = if include_metadata {
                    self.should_animate_metadata(state.art_url.as_deref(), &state.metadata)
                } else {
                    false
                };
                metadata::update_metadata_widgets(
                    &self.metadata_widgets,
                    &self.config.metadata,
                    rendered,
                    animate_metadata,
                );

                self.window.set_visible(true);
                self.reassert_layer_surface();
            }
            _ => {
                *self.playback_clock.borrow_mut() = None;

                let now = Instant::now();
                let should_clear = {
                    let mut miss_since = self.media_miss_since.borrow_mut();
                    match *miss_since {
                        Some(started) => now.saturating_duration_since(started) >= MEDIA_MISS_GRACE,
                        None => {
                            *miss_since = Some(now);
                            false
                        }
                    }
                };

                if should_clear {
                    self.handle_empty_state();
                } else {
                    self.hold_previous_cover();
                }
            }
        }
    }

    fn handle_empty_state(&self) {
        clear_artwork(
            &self.primary_artwork,
            &self.secondary_artwork,
            &self.active_slot,
            &self.transition_source,
            &self.config,
        );
        metadata::clear_metadata_widgets(&self.metadata_widgets);
        *self.current_url.borrow_mut() = None;
        *self.last_track_identity.borrow_mut() = None;
        *self.last_media_state.borrow_mut() = None;
        *self.playback_clock.borrow_mut() = None;

        if !*self.splash_active.borrow() {
            self.window.set_visible(false);
        }
    }

    fn hold_previous_cover(&self) {
        if self.current_url.borrow().is_some() {
            self.window.set_visible(true);
            self.reassert_layer_surface();
        } else {
            self.handle_empty_state();
        }
    }

    fn reassert_layer_surface(&self) {
        if self.window.is_visible() && matches!(self.config.layer, ShellLayer::Background) {
            self.window.present();
        }
    }

    fn should_animate_metadata(
        &self,
        art_url: Option<&str>,
        metadata: &crate::model::TrackMetadata,
    ) -> bool {
        let identity = TrackIdentity::from_metadata(art_url, metadata);
        let mut previous = self.last_track_identity.borrow_mut();

        if previous.as_ref() == Some(&identity) {
            false
        } else {
            *previous = Some(identity);
            true
        }
    }

    fn sync_playback_clock(&self, state: &MediaState) {
        if state.status != PlaybackStatus::Playing {
            *self.playback_clock.borrow_mut() = None;
            return;
        }

        let anchor_position_microseconds = state
            .metadata
            .position_microseconds
            .or_else(|| parse_timestamp_microseconds(&state.metadata.position));

        let Some(anchor_position_microseconds) = anchor_position_microseconds else {
            *self.playback_clock.borrow_mut() = None;
            return;
        };

        let length_microseconds = state
            .metadata
            .length_microseconds
            .or_else(|| parse_timestamp_microseconds(&state.metadata.length));

        let track = TrackIdentity::from_metadata(state.art_url.as_deref(), &state.metadata);
        let mut clock_slot = self.playback_clock.borrow_mut();
        let existing = clock_slot.take();
        *clock_slot = Some(PlaybackClock::sync(
            existing,
            track,
            anchor_position_microseconds,
            length_microseconds,
        ));
    }

    fn tick_position_display(&self) {
        let include_metadata = self.config.metadata.enabled
            && (self.config.metadata.top.enabled || self.config.metadata.left.enabled);

        if !include_metadata {
            return;
        }

        let Some(clock) = self.playback_clock.borrow().as_ref().cloned() else {
            return;
        };

        let mut state = match self.last_media_state.borrow().as_ref().cloned() {
            Some(state) => state,
            None => return,
        };

        if state.status != PlaybackStatus::Playing {
            return;
        }

        let state_track = TrackIdentity::from_metadata(state.art_url.as_deref(), &state.metadata);
        if &state_track != clock.track() {
            return;
        }

        let position_microseconds = clock.clamped_position_microseconds_now();
        let position = format_timestamp_microseconds(position_microseconds);
        if state.metadata.position == position {
            return;
        }

        state.metadata.position = position;
        state.metadata.position_microseconds = Some(position_microseconds);
        *self.last_media_state.borrow_mut() = Some(state.clone());

        let rendered = metadata::render_metadata(&self.config.metadata, &state.metadata);
        metadata::update_metadata_widgets(
            &self.metadata_widgets,
            &self.config.metadata,
            rendered,
            false,
        );
    }
}
