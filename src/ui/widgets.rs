use gtk::prelude::*;

use crate::model::{ArtworkFit, Config};

#[derive(Clone)]
pub(crate) struct ArtworkLayer {
    pub(crate) stage: gtk::Fixed,
    pub(crate) picture: gtk::Picture,
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

fn content_fit_for_artwork_fit(fit: ArtworkFit) -> gtk::ContentFit {
    match fit {
        ArtworkFit::Contain => gtk::ContentFit::Contain,
        ArtworkFit::Cover => gtk::ContentFit::Cover,
        ArtworkFit::Fill => gtk::ContentFit::Fill,
    }
}
