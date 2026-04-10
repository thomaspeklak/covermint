use gtk::{gdk, prelude::*};
use gtk4_layer_shell::{Edge, LayerShell};
use std::{cell::RefCell, rc::Rc};

use crate::{
    model::{AxisPlacement, Config, LyricsLayout, Placement},
    monitor::{monitor_label, select_monitor},
};

pub(super) fn cover_frame_size(config: &Config) -> (i32, i32) {
    let border = config.border_width.max(0) * 2;
    (config.width + border, config.height + border)
}

pub(super) fn metadata_band_sizes(config: &Config) -> (i32, i32) {
    if !config.metadata.enabled {
        return (0, 0);
    }

    let left_width = if config.metadata.left.enabled {
        config.metadata.left.band_size_px.max(0)
    } else {
        0
    };

    let top_height = if config.metadata.top.enabled {
        config.metadata.top.band_size_px.max(0)
    } else {
        0
    };

    (left_width, top_height)
}

const LYRICS_PANEL_GAP_PX: i32 = 8;

pub(super) fn panel_base_size(config: &Config) -> (i32, i32) {
    let (cover_width, cover_height) = cover_frame_size(config);
    let (left_width, top_height) = metadata_band_sizes(config);
    (cover_width + left_width, cover_height + top_height)
}

pub(super) fn lyrics_frame_size(config: &Config) -> (i32, i32) {
    let (base_width, base_height) = panel_base_size(config);
    match config.lyrics.layout {
        LyricsLayout::SingleLine => (base_width, lyrics_singleline_height(config)),
        LyricsLayout::MultiLine => (config.lyrics.panel_width.max(120), base_height),
    }
}

pub(super) fn lyrics_panel_left_width(config: &Config) -> i32 {
    if matches!(config.lyrics.layout, LyricsLayout::MultiLine) {
        config.lyrics.panel_width.max(120) + LYRICS_PANEL_GAP_PX
    } else {
        0
    }
}

pub(super) fn lyrics_panel_bottom_height(config: &Config) -> i32 {
    if matches!(config.lyrics.layout, LyricsLayout::SingleLine) {
        lyrics_singleline_height(config)
    } else {
        0
    }
}

pub(super) fn layout_window_size(config: &Config) -> (i32, i32) {
    let (base_width, base_height) = panel_base_size(config);
    (
        base_width + lyrics_panel_left_width(config),
        base_height + lyrics_panel_bottom_height(config),
    )
}

fn lyrics_singleline_height(config: &Config) -> i32 {
    let style = &config.lyrics.style;
    (style.font_size_px.max(1) + style.padding_px.max(0) * 2 + 8).max(40)
}

pub(super) fn sync_window_target(
    window: &gtk::ApplicationWindow,
    config: &Config,
    monitor_status: &Rc<RefCell<Option<String>>>,
) {
    let selected_monitor = select_monitor(&config.monitor_selector);

    if let Some(monitor) = selected_monitor.as_ref() {
        window.set_monitor(Some(monitor));
        let label = monitor_label(monitor);
        if monitor_status.borrow().as_ref() != Some(&label) {
            eprintln!("covermint: using monitor {label}");
            *monitor_status.borrow_mut() = Some(label);
        }
    } else if monitor_status.borrow().as_deref() != Some("<compositor>") {
        eprintln!(
            "covermint: monitor selector '{}' not found, compositor will choose",
            config.monitor_selector
        );
        *monitor_status.borrow_mut() = Some("<compositor>".to_string());
    }

    apply_placement(window, config, selected_monitor.as_ref());
}

fn apply_placement(
    window: &gtk::ApplicationWindow,
    config: &Config,
    monitor: Option<&gdk::Monitor>,
) {
    reset_window_position(window);

    if let Some(monitor) = monitor {
        let geometry = monitor.geometry();
        let (window_width, window_height) = layout_window_size(config);
        let x = axis_offset(
            config.placement.horizontal(),
            geometry.width(),
            window_width,
            config.offset_x,
        );
        let y = axis_offset(
            config.placement.vertical(),
            geometry.height(),
            window_height,
            config.offset_y,
        );

        set_window_anchor_and_margin(window, Edge::Left, Edge::Top, x, y);
        return;
    }

    apply_anchor_fallback(window, config);
}

fn reset_window_position(window: &gtk::ApplicationWindow) {
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, false);
        window.set_margin(edge, 0);
    }
}

fn set_window_anchor_and_margin(
    window: &gtk::ApplicationWindow,
    horizontal_edge: Edge,
    vertical_edge: Edge,
    x: i32,
    y: i32,
) {
    window.set_anchor(horizontal_edge, true);
    window.set_anchor(vertical_edge, true);
    window.set_margin(horizontal_edge, x);
    window.set_margin(vertical_edge, y);
}

fn axis_offset(alignment: AxisPlacement, available: i32, size: i32, offset: i32) -> i32 {
    match alignment {
        AxisPlacement::Start => offset,
        AxisPlacement::Center => ((available - size) / 2) + offset,
        AxisPlacement::End => available - size - offset,
    }
}

fn apply_anchor_fallback(window: &gtk::ApplicationWindow, config: &Config) {
    let (horizontal_edge, vertical_edge) = match fallback_anchor_edges(config.placement) {
        Some(edges) => edges,
        None => {
            eprintln!(
                "covermint: placement '{}' needs monitor geometry; falling back to top-left because the monitor could not be resolved",
                config.placement.label()
            );
            (Edge::Left, Edge::Top)
        }
    };

    set_window_anchor_and_margin(
        window,
        horizontal_edge,
        vertical_edge,
        config.offset_x,
        config.offset_y,
    );
}

fn fallback_anchor_edges(placement: Placement) -> Option<(Edge, Edge)> {
    match placement {
        Placement::TopLeft => Some((Edge::Left, Edge::Top)),
        Placement::TopRight => Some((Edge::Right, Edge::Top)),
        Placement::BottomLeft => Some((Edge::Left, Edge::Bottom)),
        Placement::BottomRight => Some((Edge::Right, Edge::Bottom)),
        Placement::Top
        | Placement::Left
        | Placement::Center
        | Placement::Right
        | Placement::Bottom => None,
    }
}
