mod frame;

use gtk::{gdk, glib, prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{
    model::{ArtworkSlot, Config, Transition},
    ui::ArtworkLayer,
};

use self::frame::{
    bring_artwork_to_front, render_picture_frame, reset_artwork_frame, transition_frames,
};

pub(crate) fn set_artwork_texture(
    artwork_stack: &gtk::Fixed,
    primary: &ArtworkLayer,
    secondary: &ArtworkLayer,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
    texture: &gdk::Texture,
) {
    stop_transition(transition_source);

    let animate = active_artwork_pair(primary, secondary, *active_slot.borrow())
        .0
        .picture
        .paintable()
        .is_some();

    if !animate || config.transition == Transition::None || config.transition_ms == 0 {
        set_artwork_texture_immediate(primary, secondary, *active_slot.borrow(), config, texture);
        return;
    }

    animate_artwork_transition(
        artwork_stack,
        primary,
        secondary,
        active_slot,
        transition_source,
        config,
        texture,
    );
}

pub(crate) fn clear_artwork(
    primary: &ArtworkLayer,
    secondary: &ArtworkLayer,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
) {
    stop_transition(transition_source);

    clear_picture(primary, config.width, config.height);
    clear_picture(secondary, config.width, config.height);
    primary.picture.set_opacity(1.0);
    *active_slot.borrow_mut() = ArtworkSlot::Primary;
}

fn active_artwork_pair(
    primary: &ArtworkLayer,
    secondary: &ArtworkLayer,
    slot: ArtworkSlot,
) -> (ArtworkLayer, ArtworkLayer) {
    match slot {
        ArtworkSlot::Primary => (primary.clone(), secondary.clone()),
        ArtworkSlot::Secondary => (secondary.clone(), primary.clone()),
    }
}

fn stop_transition(transition_source: &Rc<RefCell<Option<glib::SourceId>>>) {
    if let Some(source_id) = transition_source.borrow_mut().take() {
        source_id.remove();
    }
}

fn clear_picture(artwork: &ArtworkLayer, width: i32, height: i32) {
    artwork.picture.set_paintable(Option::<&gdk::Texture>::None);
    artwork.picture.set_opacity(0.0);
    reset_artwork_frame(artwork, width, height);
}

fn set_artwork_texture_immediate(
    primary: &ArtworkLayer,
    secondary: &ArtworkLayer,
    active_slot: ArtworkSlot,
    config: &Config,
    texture: &gdk::Texture,
) {
    let (active_artwork, inactive_artwork) = active_artwork_pair(primary, secondary, active_slot);
    reset_artwork_frame(&active_artwork, config.width, config.height);
    active_artwork.picture.set_paintable(Some(texture));
    active_artwork.picture.set_opacity(1.0);
    clear_picture(&inactive_artwork, config.width, config.height);
}

fn animate_artwork_transition(
    artwork_stack: &gtk::Fixed,
    primary: &ArtworkLayer,
    secondary: &ArtworkLayer,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
    texture: &gdk::Texture,
) {
    let current_slot = *active_slot.borrow();
    let next_slot = current_slot.other();
    let (from_artwork, to_artwork) = active_artwork_pair(primary, secondary, current_slot);

    if config.transition == Transition::Cover {
        bring_artwork_to_front(artwork_stack, &to_artwork);
    }

    to_artwork.picture.set_paintable(Some(texture));
    let (from_start, to_start) = transition_frames(config.transition, config.placement, 0.0);
    render_picture_frame(
        &from_artwork,
        config.width,
        config.height,
        config.transition,
        from_start,
    );
    render_picture_frame(
        &to_artwork,
        config.width,
        config.height,
        config.transition,
        to_start,
    );

    let active_slot = active_slot.clone();
    let transition_source_for_closure = transition_source.clone();
    let start = Instant::now();
    let duration = Duration::from_millis(config.transition_ms as u64);
    let transition = config.transition;
    let width = config.width;
    let height = config.height;
    let placement = config.placement;

    let source_id = glib::timeout_add_local(Duration::from_millis(16), move || {
        let progress = (start.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
        let (from_frame, to_frame) = transition_frames(transition, placement, progress);
        render_picture_frame(&from_artwork, width, height, transition, from_frame);
        render_picture_frame(&to_artwork, width, height, transition, to_frame);

        if progress >= 1.0 {
            clear_picture(&from_artwork, width, height);
            reset_artwork_frame(&to_artwork, width, height);
            to_artwork.picture.set_opacity(1.0);
            *active_slot.borrow_mut() = next_slot;
            *transition_source_for_closure.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });

    *transition_source.borrow_mut() = Some(source_id);
}
