use gtk::{glib, prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{
    artwork::download_texture,
    metadata::{self, MetadataWidgets},
    model::{ArtworkSlot, Config, ShellLayer},
    player::query_player,
    transitions::{clear_artwork, set_artwork_texture},
};

use super::{ArtworkLayer, layout::sync_window_target};

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

    glib::timeout_add_seconds_local(state.config.poll_seconds, move || {
        state.refresh();
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

                let rendered = metadata::render_metadata(&self.config.metadata, &state.metadata);
                metadata::update_metadata_widgets(
                    &self.metadata_widgets,
                    &self.config.metadata,
                    rendered,
                );

                self.window.set_visible(true);
                self.reassert_layer_surface();
            }
            _ => {
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
}
