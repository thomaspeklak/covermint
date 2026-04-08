mod layout;
mod playback_clock;
mod runtime;
mod splash;
mod style;
mod widgets;

pub(crate) use widgets::ArtworkLayer;

use gtk::prelude::*;
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
use std::{cell::RefCell, rc::Rc, time::Instant};

use crate::{
    artwork::load_texture,
    metadata::{self, MetadataWidgets},
    model::{ArtworkSlot, Config, MetadataSection, ShellLayer},
};

use self::{
    layout::{cover_frame_size, layout_window_size, metadata_band_sizes, sync_window_target},
    runtime::{UiRefreshState, install_refresh_loop},
    splash::{
        SPLASH_LOGO, new_splash_view, schedule_startup_splash_dismissal, set_splash_texture,
        start_splash_animation,
    },
    style::install_styles,
    widgets::new_artwork_layer,
};

pub(crate) fn build_ui(app: &gtk::Application, config: Rc<Config>) {
    let (window_width, window_height) = layout_window_size(&config);
    let (cover_width, cover_height) = cover_frame_size(&config);
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("covermint")
        .resizable(false)
        .build();

    window.set_decorated(false);
    window.set_focusable(false);
    window.set_can_focus(false);
    window.set_can_target(false);
    window.set_default_size(window_width, window_height);
    window.set_size_request(window_width, window_height);
    window.add_css_class("covermint-window");

    window.init_layer_shell();
    window.set_namespace(Some("covermint"));
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_layer(match config.layer {
        ShellLayer::Background => Layer::Background,
        ShellLayer::Bottom => Layer::Bottom,
    });
    window.set_exclusive_zone(0);

    let monitor_status = Rc::new(RefCell::new(None::<String>));
    sync_window_target(&window, &config, &monitor_status);
    install_styles(&config);

    let primary_artwork = new_artwork_layer(&config);
    let secondary_artwork = new_artwork_layer(&config);
    let splash = new_splash_view(&config);
    secondary_artwork.picture.set_opacity(0.0);

    let splash_enabled = if let Some(texture) = load_texture(SPLASH_LOGO.to_vec()) {
        set_splash_texture(&splash, &texture);
        splash.stage.set_visible(true);
        start_splash_animation(&splash);
        true
    } else {
        eprintln!("covermint: failed to load embedded splash logo");
        false
    };

    let artwork_stack = gtk::Fixed::new();
    artwork_stack.set_size_request(config.width, config.height);
    artwork_stack.set_halign(gtk::Align::Fill);
    artwork_stack.set_valign(gtk::Align::Fill);
    artwork_stack.put(&primary_artwork.stage, 0.0, 0.0);
    artwork_stack.put(&secondary_artwork.stage, 0.0, 0.0);

    let overlay = gtk::Overlay::new();
    overlay.set_size_request(config.width, config.height);
    overlay.set_halign(gtk::Align::Fill);
    overlay.set_valign(gtk::Align::Fill);
    overlay.set_child(Some(&artwork_stack));
    overlay.add_overlay(&splash.stage);

    let artwork_stage = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_stage.add_css_class("covermint-artwork-stage");
    artwork_stage.set_size_request(config.width, config.height);
    artwork_stage.set_halign(gtk::Align::Center);
    artwork_stage.set_valign(gtk::Align::Center);
    artwork_stage.append(&overlay);

    let cover_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    cover_frame.add_css_class("covermint-artwork");
    cover_frame.set_size_request(cover_width, cover_height);
    cover_frame.set_halign(gtk::Align::Fill);
    cover_frame.set_valign(gtk::Align::Fill);
    cover_frame.set_opacity(config.opacity);
    cover_frame.append(&artwork_stage);

    let top_widget = if config.metadata.enabled && config.metadata.top.enabled {
        Some(metadata::new_metadata_label(
            &config.metadata.top,
            MetadataSection::Top,
            cover_width,
        ))
    } else {
        None
    };

    let left_widget = if config.metadata.enabled && config.metadata.left.enabled {
        Some(metadata::new_metadata_label(
            &config.metadata.left,
            MetadataSection::Left,
            cover_height,
        ))
    } else {
        None
    };

    let (left_band, top_band) = metadata_band_sizes(&config);

    let root = gtk::Fixed::new();
    root.set_size_request(window_width, window_height);
    root.set_halign(gtk::Align::Fill);
    root.set_valign(gtk::Align::Fill);
    root.set_overflow(gtk::Overflow::Hidden);

    root.put(&cover_frame, left_band as f64, top_band as f64);

    if left_band > 0 && top_band > 0 {
        let corner_fill = gtk::Box::new(gtk::Orientation::Vertical, 0);
        corner_fill.add_css_class("covermint-meta-corner");
        corner_fill.set_size_request(left_band, top_band);
        corner_fill.set_halign(gtk::Align::Fill);
        corner_fill.set_valign(gtk::Align::Fill);
        root.put(&corner_fill, 0.0, 0.0);
    }

    if let Some(top) = top_widget.as_ref() {
        root.put(&top.wrapper, left_band as f64, 0.0);
    }

    if let Some(left) = left_widget.as_ref() {
        root.put(&left.wrapper, 0.0, top_band as f64);
    }

    let metadata_widgets = MetadataWidgets {
        top: top_widget,
        left: left_widget,
    };

    window.set_child(Some(&root));
    window.present();
    window.set_visible(splash_enabled);

    let current_url = Rc::new(RefCell::new(None::<String>));
    let active_slot = Rc::new(RefCell::new(ArtworkSlot::Primary));
    let transition_source = Rc::new(RefCell::new(None::<gtk::glib::SourceId>));
    let splash_active = Rc::new(RefCell::new(splash_enabled));
    let media_miss_since = Rc::new(RefCell::new(None::<Instant>));
    let last_track_identity = Rc::new(RefCell::new(None::<playback_clock::TrackIdentity>));
    let last_media_state = Rc::new(RefCell::new(None::<crate::model::MediaState>));
    let playback_clock = Rc::new(RefCell::new(None::<playback_clock::PlaybackClock>));

    if splash_enabled {
        schedule_startup_splash_dismissal(&window, &splash, &splash_active, &current_url);
    }

    install_refresh_loop(UiRefreshState {
        window: window.clone(),
        config,
        monitor_status,
        artwork_stack,
        primary_artwork,
        secondary_artwork,
        metadata_widgets,
        current_url,
        active_slot,
        transition_source,
        splash_active,
        media_miss_since,
        last_track_identity,
        last_media_state,
        playback_clock,
    });
}
