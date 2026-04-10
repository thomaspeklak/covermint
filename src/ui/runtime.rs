use gtk::{glib, graphene, gsk, prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{
    artwork::download_texture,
    lyrics::{self, LyricsLookupResult, SyncedLyrics},
    metadata::{self, MetadataWidgets},
    model::{ArtworkSlot, Config, LyricsLayout, MediaState, PlaybackStatus, ShellLayer},
    player::query_player,
    timestamp::{format_timestamp_microseconds, parse_timestamp_microseconds},
    transitions::{clear_artwork, set_artwork_texture},
};

use super::{
    ArtworkLayer, LyricsWidget, LyricsWidgetMode,
    layout::sync_window_target,
    playback_clock::{POSITION_TICK_INTERVAL, PlaybackClock, TrackIdentity},
};

const MEDIA_MISS_GRACE: Duration = Duration::from_secs(5);
const MPRIS_EVENT_PUMP_INTERVAL: Duration = Duration::from_millis(200);
const CONTROL_COMMAND_PUMP_INTERVAL: Duration = Duration::from_millis(120);
const LYRICS_SCROLL_TICK_INTERVAL: Duration = Duration::from_millis(16);
const LYRICS_SCROLL_DURATION: Duration = Duration::from_millis(240);

#[derive(Clone, Copy)]
pub(super) struct LyricsScrollAnimation {
    from_index: usize,
    to_index: usize,
    started_at: Instant,
}

#[derive(Clone)]
pub(super) struct UiRefreshState {
    pub(super) window: gtk::ApplicationWindow,
    pub(super) config: Rc<Config>,
    pub(super) monitor_status: Rc<RefCell<Option<String>>>,
    pub(super) artwork_stack: gtk::Fixed,
    pub(super) primary_artwork: ArtworkLayer,
    pub(super) secondary_artwork: ArtworkLayer,
    pub(super) metadata_widgets: MetadataWidgets,
    pub(super) lyrics_widget: Option<LyricsWidget>,
    pub(super) current_url: Rc<RefCell<Option<String>>>,
    pub(super) active_slot: Rc<RefCell<ArtworkSlot>>,
    pub(super) transition_source: Rc<RefCell<Option<glib::SourceId>>>,
    pub(super) splash_active: Rc<RefCell<bool>>,
    pub(super) media_miss_since: Rc<RefCell<Option<Instant>>>,
    pub(super) last_track_identity: Rc<RefCell<Option<TrackIdentity>>>,
    pub(super) last_media_state: Rc<RefCell<Option<MediaState>>>,
    pub(super) playback_clock: Rc<RefCell<Option<PlaybackClock>>>,
    pub(super) lyrics_signature: Rc<RefCell<Option<String>>>,
    pub(super) synced_lyrics: Rc<RefCell<Option<SyncedLyrics>>>,
    pub(super) lyrics_missing: Rc<RefCell<bool>>,
    pub(super) lyrics_visible: Rc<RefCell<bool>>,
    pub(super) current_lyrics_index: Rc<RefCell<Option<usize>>>,
    pub(super) lyrics_scroll_animation: Rc<RefCell<Option<LyricsScrollAnimation>>>,
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

    let (control_tx, control_rx) = std::sync::mpsc::channel::<crate::control::ControlCommand>();
    crate::control::start_listener(control_tx);

    let control_state = state.clone();
    glib::timeout_add_local(CONTROL_COMMAND_PUMP_INTERVAL, move || {
        let mut changed = false;
        while let Ok(command) = control_rx.try_recv() {
            control_state.apply_control_command(command);
            changed = true;
        }

        if changed {
            control_state.refresh();
        }

        glib::ControlFlow::Continue
    });

    let position_tick_state = state.clone();
    glib::timeout_add_local(POSITION_TICK_INTERVAL, move || {
        position_tick_state.tick_position_display();
        glib::ControlFlow::Continue
    });

    let lyrics_tick_state = state;
    glib::timeout_add_local(LYRICS_SCROLL_TICK_INTERVAL, move || {
        lyrics_tick_state.tick_lyrics_animation();
        glib::ControlFlow::Continue
    });
}

impl UiRefreshState {
    fn refresh(&self) {
        sync_window_target(&self.window, &self.config, &self.monitor_status);

        let include_metadata = self.config.metadata.enabled
            && (self.config.metadata.top.enabled || self.config.metadata.left.enabled);
        let include_lyrics = *self.lyrics_visible.borrow() && self.lyrics_widget.is_some();
        let include_track_metadata = include_metadata || include_lyrics;

        match query_player(&self.config.player, include_track_metadata) {
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
                if include_track_metadata {
                    self.sync_playback_clock(&state);
                } else {
                    *self.playback_clock.borrow_mut() = None;
                }

                if include_metadata {
                    let rendered =
                        metadata::render_metadata(&self.config.metadata, &state.metadata);
                    let animate_metadata =
                        self.should_animate_metadata(state.art_url.as_deref(), &state.metadata);
                    metadata::update_metadata_widgets(
                        &self.metadata_widgets,
                        &self.config.metadata,
                        rendered,
                        animate_metadata,
                    );
                } else {
                    metadata::clear_metadata_widgets(&self.metadata_widgets);
                }

                self.update_lyrics(&state, include_lyrics);

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
        *self.lyrics_signature.borrow_mut() = None;
        *self.synced_lyrics.borrow_mut() = None;
        *self.lyrics_missing.borrow_mut() = false;
        *self.current_lyrics_index.borrow_mut() = None;
        *self.lyrics_scroll_animation.borrow_mut() = None;
        self.set_lyrics_status("");

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

    fn update_lyrics(&self, state: &MediaState, include_lyrics: bool) {
        if !include_lyrics {
            *self.lyrics_signature.borrow_mut() = None;
            *self.synced_lyrics.borrow_mut() = None;
            *self.lyrics_missing.borrow_mut() = false;
            *self.current_lyrics_index.borrow_mut() = None;
            *self.lyrics_scroll_animation.borrow_mut() = None;
            self.set_lyrics_status("");
            return;
        }

        let Some(signature) = lyrics::signature_from_metadata(&state.metadata) else {
            *self.lyrics_signature.borrow_mut() = None;
            *self.synced_lyrics.borrow_mut() = None;
            *self.lyrics_missing.borrow_mut() = true;
            *self.current_lyrics_index.borrow_mut() = None;
            *self.lyrics_scroll_animation.borrow_mut() = None;
            self.set_lyrics_status("");
            self.set_lyrics_frame_faded(matches!(
                self.config.lyrics.layout,
                LyricsLayout::MultiLine
            ));
            return;
        };

        let needs_lookup = self
            .lyrics_signature
            .borrow()
            .as_ref()
            .map(|previous| previous != signature.cache_key())
            .unwrap_or(true);

        if needs_lookup {
            *self.lyrics_signature.borrow_mut() = Some(signature.cache_key().to_string());
            *self.synced_lyrics.borrow_mut() = None;
            *self.lyrics_missing.borrow_mut() = false;

            match lyrics::lookup_synced_lyrics(&signature, true) {
                LyricsLookupResult::Found(lines) => {
                    *self.synced_lyrics.borrow_mut() = Some(lines);
                    *self.lyrics_missing.borrow_mut() = false;
                }
                LyricsLookupResult::Missing => {
                    *self.lyrics_missing.borrow_mut() = true;
                }
                LyricsLookupResult::NotLoaded => {}
            }
        }

        let position_microseconds = state
            .metadata
            .position_microseconds
            .or_else(|| parse_timestamp_microseconds(&state.metadata.position))
            .unwrap_or(0);

        self.render_lyrics_line(position_microseconds);
    }

    fn render_lyrics_line(&self, position_microseconds: u64) {
        if let Some(lyrics) = self.synced_lyrics.borrow().as_ref() {
            self.set_lyrics_frame_faded(false);

            if let Some(current_index) = lyrics.current_line_index(position_microseconds) {
                let shift_lines = self.scroll_shift_lines(current_index);
                self.render_lyrics_with_focus(lyrics, current_index, shift_lines);
                return;
            }

            *self.current_lyrics_index.borrow_mut() = None;
            *self.lyrics_scroll_animation.borrow_mut() = None;
            if position_microseconds == 0 {
                self.set_lyrics_status("♪");
            } else {
                self.set_lyrics_status("");
            }
            return;
        }

        *self.current_lyrics_index.borrow_mut() = None;
        *self.lyrics_scroll_animation.borrow_mut() = None;

        if *self.lyrics_missing.borrow() {
            self.set_lyrics_status("");
            self.set_lyrics_frame_faded(matches!(
                self.config.lyrics.layout,
                LyricsLayout::MultiLine
            ));
            return;
        }

        self.set_lyrics_frame_faded(false);
        self.set_lyrics_status("");
    }

    fn scroll_shift_lines(&self, current_index: usize) -> f64 {
        let mut current_slot = self.current_lyrics_index.borrow_mut();
        let previous = *current_slot;

        if !self.config.lyrics.smooth_scroll {
            *current_slot = Some(current_index);
            *self.lyrics_scroll_animation.borrow_mut() = None;
            return 0.0;
        }

        if previous != Some(current_index) {
            if let Some(previous_index) = previous {
                let delta = current_index as isize - previous_index as isize;
                if delta.abs() == 1 {
                    *self.lyrics_scroll_animation.borrow_mut() = Some(LyricsScrollAnimation {
                        from_index: previous_index,
                        to_index: current_index,
                        started_at: Instant::now(),
                    });
                } else {
                    *self.lyrics_scroll_animation.borrow_mut() = None;
                }
            } else {
                *self.lyrics_scroll_animation.borrow_mut() = None;
            }
            *current_slot = Some(current_index);
        }

        let mut animation_slot = self.lyrics_scroll_animation.borrow_mut();
        let Some(animation) = *animation_slot else {
            return 0.0;
        };

        if animation.to_index != current_index {
            *animation_slot = None;
            return 0.0;
        }

        let elapsed = animation.started_at.elapsed();
        if elapsed >= LYRICS_SCROLL_DURATION {
            *animation_slot = None;
            return 0.0;
        }

        let t = elapsed.as_secs_f64() / LYRICS_SCROLL_DURATION.as_secs_f64();
        let line_delta = (animation.to_index as isize - animation.from_index as isize) as f64;
        (1.0 - t).clamp(0.0, 1.0) * line_delta.clamp(-1.0, 1.0)
    }

    fn set_lyrics_status(&self, text: &str) {
        let Some(lyrics_widget) = self.lyrics_widget.as_ref() else {
            return;
        };

        match &lyrics_widget.mode {
            LyricsWidgetMode::SingleLine {
                label,
                current_text,
            } => {
                if current_text.borrow().as_str() == text {
                    return;
                }

                label.set_label(text);
                *current_text.borrow_mut() = text.to_string();
            }
            LyricsWidgetMode::MultiLine {
                stage,
                container,
                labels,
                center_slot,
                ..
            } => {
                for (index, label) in labels.iter().enumerate() {
                    if index == *center_slot {
                        label.set_label(text);
                        label.set_opacity(1.0);
                        label.add_css_class("covermint-lyrics-line-current");
                    } else {
                        label.set_label("");
                        label.set_opacity(0.0);
                        label.remove_css_class("covermint-lyrics-line-current");
                    }
                }

                self.set_multiline_shift(stage, container, 0.0);
            }
        }
    }

    fn render_lyrics_with_focus(
        &self,
        lyrics: &SyncedLyrics,
        current_index: usize,
        shift_lines: f64,
    ) {
        let Some(lyrics_widget) = self.lyrics_widget.as_ref() else {
            return;
        };

        match &lyrics_widget.mode {
            LyricsWidgetMode::SingleLine { .. } => {
                let line = lyrics.line_text(current_index).unwrap_or("♪");
                self.set_lyrics_status(line);
            }
            LyricsWidgetMode::MultiLine {
                stage,
                container,
                labels,
                center_slot,
                line_height_px,
            } => {
                let fade_radius = (*center_slot).max(1) as f64 + 0.45;

                for (slot_index, label) in labels.iter().enumerate() {
                    let relative = slot_index as isize - *center_slot as isize;
                    let lyric_index = current_index as isize + relative;
                    let text = if lyric_index >= 0 {
                        lyrics.line_text(lyric_index as usize).unwrap_or("")
                    } else {
                        ""
                    };

                    label.set_label(text);

                    if relative == 0 {
                        label.add_css_class("covermint-lyrics-line-current");
                    } else {
                        label.remove_css_class("covermint-lyrics-line-current");
                    }

                    let distance = ((relative as f64) - shift_lines).abs();
                    let opacity = if text.is_empty() {
                        0.0
                    } else {
                        (1.0 - (distance / fade_radius)).clamp(0.18, 1.0)
                    };
                    label.set_opacity(opacity);
                }

                let row_height = labels
                    .get(*center_slot)
                    .map(|label| label.height().max(*line_height_px))
                    .unwrap_or(*line_height_px);
                let shift_pixels = shift_lines * f64::from(row_height);
                self.set_multiline_shift(stage, container, shift_pixels);
            }
        }
    }

    fn set_multiline_shift(&self, stage: &gtk::Fixed, container: &gtk::Box, y_pixels: f64) {
        let transform =
            gsk::Transform::new().translate(&graphene::Point::new(0.0, y_pixels as f32));
        stage.set_child_transform(container, Some(&transform));
    }

    fn set_lyrics_frame_faded(&self, faded: bool) {
        let Some(lyrics_widget) = self.lyrics_widget.as_ref() else {
            return;
        };

        let target = if faded { 0.0 } else { 1.0 };
        if (lyrics_widget.frame.opacity() - target).abs() > f64::EPSILON {
            lyrics_widget.frame.set_opacity(target);
        }
    }

    fn apply_control_command(&self, command: crate::control::ControlCommand) {
        let next_visible = match command {
            crate::control::ControlCommand::LyricsOn => true,
            crate::control::ControlCommand::LyricsOff => false,
            crate::control::ControlCommand::LyricsToggle => !*self.lyrics_visible.borrow(),
        };

        let mut visible = self.lyrics_visible.borrow_mut();
        if *visible == next_visible {
            return;
        }
        *visible = next_visible;

        if let Some(widget) = self.lyrics_widget.as_ref() {
            widget.frame.set_visible(next_visible);
        }

        if !next_visible {
            *self.current_lyrics_index.borrow_mut() = None;
            *self.lyrics_scroll_animation.borrow_mut() = None;
            self.set_lyrics_status("");
        }
    }

    fn tick_lyrics_animation(&self) {
        if !*self.lyrics_visible.borrow() {
            return;
        }

        if self.lyrics_scroll_animation.borrow().is_none() {
            return;
        }

        let position_microseconds = if let Some(clock) = self.playback_clock.borrow().as_ref() {
            clock.clamped_position_microseconds_now()
        } else {
            let Some(state) = self.last_media_state.borrow().as_ref().cloned() else {
                return;
            };

            state
                .metadata
                .position_microseconds
                .or_else(|| parse_timestamp_microseconds(&state.metadata.position))
                .unwrap_or(0)
        };

        self.render_lyrics_line(position_microseconds);
    }

    fn tick_position_display(&self) {
        let include_metadata = self.config.metadata.enabled
            && (self.config.metadata.top.enabled || self.config.metadata.left.enabled);
        let include_lyrics = *self.lyrics_visible.borrow() && self.lyrics_widget.is_some();

        if !include_metadata && !include_lyrics {
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
            if include_lyrics {
                self.render_lyrics_line(position_microseconds);
            }
            return;
        }

        state.metadata.position = position;
        state.metadata.position_microseconds = Some(position_microseconds);
        *self.last_media_state.borrow_mut() = Some(state.clone());

        if include_metadata {
            let rendered = metadata::render_metadata(&self.config.metadata, &state.metadata);
            metadata::update_metadata_widgets(
                &self.metadata_widgets,
                &self.config.metadata,
                rendered,
                false,
            );
        }

        if include_lyrics {
            self.render_lyrics_line(position_microseconds);
        }
    }
}
