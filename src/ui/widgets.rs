use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};

use crate::model::{ArtworkFit, Config, LyricsLayout};

#[derive(Clone)]
pub(crate) struct ArtworkLayer {
    pub(crate) stage: gtk::Fixed,
    pub(crate) picture: gtk::Picture,
}

#[derive(Clone)]
pub(crate) struct LyricsWidget {
    pub(crate) frame: gtk::Box,
    pub(crate) mode: LyricsWidgetMode,
}

#[derive(Clone)]
pub(crate) enum LyricsWidgetMode {
    SingleLine {
        label: gtk::Label,
        current_text: Rc<RefCell<String>>,
    },
    MultiLine {
        stage: gtk::Fixed,
        container: gtk::Box,
        labels: Vec<gtk::Label>,
        center_slot: usize,
        line_height_px: i32,
    },
}

pub(super) fn new_artwork_layer(config: &Config) -> ArtworkLayer {
    let picture = new_artwork_picture(config);
    let stage = gtk::Fixed::new();
    stage.set_size_request(config.width, config.height);
    stage.set_halign(gtk::Align::Fill);
    stage.set_valign(gtk::Align::Fill);
    stage.put(&picture, 0.0, 0.0);

    ArtworkLayer { stage, picture }
}

fn new_artwork_picture(config: &Config) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_size_request(config.width, config.height);
    picture.set_can_shrink(true);
    picture.set_content_fit(content_fit_for_artwork_fit(config.artwork_fit));
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
    picture
}

pub(super) fn new_lyrics_widget(config: &Config, width: i32, height: i32) -> LyricsWidget {
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("covermint-lyrics-frame");
    frame.set_size_request(width.max(1), height.max(1));
    frame.set_halign(gtk::Align::Fill);
    frame.set_valign(gtk::Align::Fill);
    frame.set_hexpand(true);
    frame.set_overflow(gtk::Overflow::Hidden);

    let mode = match config.lyrics.layout {
        LyricsLayout::SingleLine => {
            let label = gtk::Label::new(None);
            label.add_css_class("covermint-lyrics-label");
            label.set_halign(gtk::Align::Fill);
            label.set_valign(gtk::Align::Center);
            label.set_justify(gtk::Justification::Center);
            label.set_wrap(true);
            label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            label.set_xalign(0.5);
            label.set_yalign(0.5);
            label.set_single_line_mode(false);
            frame.append(&label);

            LyricsWidgetMode::SingleLine {
                label,
                current_text: Rc::new(RefCell::new(String::new())),
            }
        }
        LyricsLayout::MultiLine => {
            let lines_visible = config.lyrics.lines_visible.max(1);
            let lines_visible = if lines_visible % 2 == 0 {
                lines_visible + 1
            } else {
                lines_visible
            };
            let center_slot = lines_visible / 2;
            let line_height_px = ((config.lyrics.style.font_size_px.max(1) as f64) * 1.35)
                .round()
                .max(20.0) as i32;

            let container = gtk::Box::new(gtk::Orientation::Vertical, 2);
            container.add_css_class("covermint-lyrics-multiline");
            container.set_size_request(width.max(1), height.max(1));
            container.set_halign(gtk::Align::Fill);
            container.set_valign(gtk::Align::Center);
            container.set_hexpand(true);
            container.set_vexpand(true);

            let mut labels = Vec::with_capacity(lines_visible);
            for index in 0..lines_visible {
                let label = gtk::Label::new(None);
                label.add_css_class("covermint-lyrics-line");
                label.set_halign(gtk::Align::Fill);
                label.set_hexpand(true);
                label.set_valign(gtk::Align::Center);
                label.set_xalign(0.0);
                label.set_wrap(true);
                label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                label.set_single_line_mode(false);

                if index == center_slot {
                    label.add_css_class("covermint-lyrics-line-current");
                }

                container.append(&label);
                labels.push(label);
            }

            let stage = gtk::Fixed::new();
            stage.set_size_request(width.max(1), height.max(1));
            stage.set_halign(gtk::Align::Fill);
            stage.set_valign(gtk::Align::Fill);
            stage.set_hexpand(true);
            stage.set_vexpand(true);
            stage.set_overflow(gtk::Overflow::Hidden);
            stage.put(&container, 0.0, 0.0);

            frame.append(&stage);

            LyricsWidgetMode::MultiLine {
                stage,
                container,
                labels,
                center_slot,
                line_height_px,
            }
        }
    };

    if !config.lyrics.enabled {
        frame.set_visible(false);
    }

    LyricsWidget { frame, mode }
}

fn content_fit_for_artwork_fit(fit: ArtworkFit) -> gtk::ContentFit {
    match fit {
        ArtworkFit::Contain => gtk::ContentFit::Contain,
        ArtworkFit::Cover => gtk::ContentFit::Cover,
        ArtworkFit::Fill => gtk::ContentFit::Fill,
    }
}
