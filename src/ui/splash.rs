use gtk::{glib, prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{model::Config, motion::ease_in_out_cubic};

pub(super) const SPLASH_LOGO: &[u8] =
    include_bytes!("../../assets/branding/covermint-logo-grunge.png");

const STARTUP_SPLASH_MIN_SHOW: Duration = Duration::from_millis(900);
const STARTUP_SPLASH_FADE: Duration = Duration::from_millis(220);

pub(super) fn new_splash_picture(config: &Config) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_size_request(
        (config.width * 2 / 3).max(1),
        (config.height * 2 / 3).max(1),
    );
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(false);
    picture.set_vexpand(false);
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
    picture.set_visible(false);
    picture
}

pub(super) fn schedule_startup_splash_dismissal(
    window: &gtk::ApplicationWindow,
    splash_picture: &gtk::Picture,
    splash_active: &Rc<RefCell<bool>>,
    current_url: &Rc<RefCell<Option<String>>>,
) {
    let window = window.clone();
    let splash_picture = splash_picture.clone();
    let splash_active = splash_active.clone();
    let current_url = current_url.clone();

    glib::timeout_add_local_once(STARTUP_SPLASH_MIN_SHOW, move || {
        dismiss_startup_splash(&window, &splash_picture, &splash_active, &current_url);
    });
}

fn dismiss_startup_splash(
    window: &gtk::ApplicationWindow,
    splash_picture: &gtk::Picture,
    splash_active: &Rc<RefCell<bool>>,
    current_url: &Rc<RefCell<Option<String>>>,
) {
    let mut active = splash_active.borrow_mut();
    if !*active {
        return;
    }
    *active = false;
    drop(active);

    let window = window.clone();
    let splash_picture = splash_picture.clone();
    let current_url = current_url.clone();
    let start = Instant::now();

    glib::timeout_add_local(Duration::from_millis(16), move || {
        let progress = (start.elapsed().as_secs_f64() / STARTUP_SPLASH_FADE.as_secs_f64()).min(1.0);
        splash_picture.set_opacity(1.0 - ease_in_out_cubic(progress));

        if progress >= 1.0 {
            splash_picture.set_visible(false);
            splash_picture.set_opacity(1.0);

            if current_url.borrow().is_none() {
                window.set_visible(false);
            }

            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });
}
