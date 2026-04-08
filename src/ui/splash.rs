use gtk::{gdk, glib, prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{model::Config, motion::ease_in_out_cubic};

pub(super) const SPLASH_LOGO: &[u8] =
    include_bytes!("../../assets/branding/covermint-logo-grunge.png");

const STARTUP_SPLASH_MIN_SHOW: Duration = Duration::from_millis(1_200);
const STARTUP_SPLASH_POWER_OFF: Duration = Duration::from_millis(520);
const SPLASH_FRAME_INTERVAL: Duration = Duration::from_millis(16);
const SPLASH_SCANLINE_STRIDE: i32 = 3;
const CRT_LINE_HEIGHT: f64 = 2.0;
const CRT_VERTICAL_COLLAPSE_PORTION: f64 = 0.78;

#[derive(Clone)]
pub(super) struct SplashView {
    pub(super) stage: gtk::Fixed,
    logo_stage: gtk::Fixed,
    base: gtk::Picture,
    ghost_a: gtk::Picture,
    ghost_b: gtk::Picture,
    scanlines: gtk::DrawingArea,
    power_dot: gtk::DrawingArea,
    power_dot_strength: Rc<RefCell<f64>>,
    logo_width: i32,
    logo_height: i32,
    animation_source: Rc<RefCell<Option<glib::SourceId>>>,
}

struct PowerOffFrame {
    width: i32,
    height: i32,
    stage_opacity: f64,
    line_opacity: f64,
    ghost_opacity: f64,
    scanline_opacity: f64,
    dot_strength: f64,
}

pub(super) fn new_splash_view(config: &Config) -> SplashView {
    let splash_width = config.width.max(1);
    let splash_height = config.height.max(1);
    let logo_width = (config.width * 2 / 3).max(1);
    let logo_height = (config.height * 2 / 3).max(1);

    let stage = gtk::Fixed::new();
    stage.set_size_request(splash_width, splash_height);
    stage.set_halign(gtk::Align::Fill);
    stage.set_valign(gtk::Align::Fill);
    stage.set_hexpand(true);
    stage.set_vexpand(true);
    stage.set_can_target(false);
    stage.set_visible(false);

    let logo_stage = gtk::Fixed::new();
    logo_stage.set_size_request(logo_width, logo_height);
    logo_stage.set_can_target(false);
    logo_stage.set_overflow(gtk::Overflow::Hidden);

    let logo_x = ((splash_width - logo_width) / 2).max(0) as f64;
    let logo_y = ((splash_height - logo_height) / 2).max(0) as f64;
    stage.put(&logo_stage, logo_x, logo_y);

    let base = new_splash_layer(logo_width, logo_height);
    let ghost_a = new_splash_layer(logo_width, logo_height);
    let ghost_b = new_splash_layer(logo_width, logo_height);
    let scanlines = new_scanline_layer(logo_width, logo_height);

    logo_stage.put(&base, 0.0, 0.0);
    logo_stage.put(&ghost_a, 0.0, 0.0);
    logo_stage.put(&ghost_b, 0.0, 0.0);
    logo_stage.put(&scanlines, 0.0, 0.0);

    let power_dot_strength = Rc::new(RefCell::new(0.0));
    let dot_size = ((logo_height as f64 * 0.20).round() as i32).clamp(24, 84);
    let power_dot = new_power_dot_layer(dot_size, power_dot_strength.clone());
    let dot_x = ((splash_width - dot_size) / 2).max(0) as f64;
    let dot_y = ((splash_height - dot_size) / 2).max(0) as f64;
    stage.put(&power_dot, dot_x, dot_y);

    ghost_a.set_opacity(0.0);
    ghost_b.set_opacity(0.0);
    scanlines.set_opacity(0.22);
    power_dot.set_opacity(0.0);

    SplashView {
        stage,
        logo_stage,
        base,
        ghost_a,
        ghost_b,
        scanlines,
        power_dot,
        power_dot_strength,
        logo_width,
        logo_height,
        animation_source: Rc::new(RefCell::new(None)),
    }
}

pub(super) fn set_splash_texture(splash: &SplashView, texture: &gdk::Texture) {
    splash.base.set_paintable(Some(texture));
    splash.ghost_a.set_paintable(Some(texture));
    splash.ghost_b.set_paintable(Some(texture));
}

pub(super) fn start_splash_animation(splash: &SplashView) {
    stop_splash_animation(splash);
    reset_splash_animation_state(splash);
    splash.stage.set_opacity(1.0);

    let stage = splash.stage.clone();
    let logo_stage = splash.logo_stage.clone();
    let base = splash.base.clone();
    let ghost_a = splash.ghost_a.clone();
    let ghost_b = splash.ghost_b.clone();
    let scanlines = splash.scanlines.clone();
    let animation_source = splash.animation_source.clone();
    let start = Instant::now();

    let source_id = glib::timeout_add_local(SPLASH_FRAME_INTERVAL, move || {
        if !stage.is_visible() {
            *animation_source.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }

        let elapsed = start.elapsed().as_secs_f64();
        let burst = (elapsed * 7.4).sin().abs().powf(18.0);
        let jitter =
            ((elapsed * 43.0).sin() * 1.4 + (elapsed * 97.0).sin() * 0.6) * (1.0 + burst * 5.0);
        let vertical = (elapsed * 31.0).sin() * (0.6 + burst * 2.2);

        let base_x = ((elapsed * 29.0).sin() * burst * 5.5).round();
        let base_y = (vertical * 0.5).round();
        let ghost_a_x = (jitter + burst * 8.5).round();
        let ghost_b_x = (-jitter * 0.75 - burst * 6.0).round();
        let ghost_a_y = (vertical * 0.7).round();
        let ghost_b_y = (-vertical * 0.45).round();
        let scanline_y = ((elapsed * 85.0).sin() * (0.4 + burst * 1.6)).round();

        logo_stage.move_(&base, base_x, base_y);
        logo_stage.move_(&ghost_a, ghost_a_x, ghost_a_y);
        logo_stage.move_(&ghost_b, ghost_b_x, ghost_b_y);
        logo_stage.move_(&scanlines, 0.0, scanline_y);

        let flicker =
            0.78 + (elapsed * 24.0).sin() * 0.08 + (elapsed * 61.0).sin() * 0.05 + burst * 0.09;

        base.set_opacity(flicker.clamp(0.58, 1.0));
        ghost_a.set_opacity((0.14 + burst * 0.34).clamp(0.10, 0.52));
        ghost_b.set_opacity((0.10 + burst * 0.28).clamp(0.08, 0.45));
        scanlines
            .set_opacity((0.24 + (elapsed * 13.0).sin() * 0.06 + burst * 0.20).clamp(0.16, 0.56));

        glib::ControlFlow::Continue
    });

    *splash.animation_source.borrow_mut() = Some(source_id);
}

pub(super) fn schedule_startup_splash_dismissal(
    window: &gtk::ApplicationWindow,
    splash: &SplashView,
    splash_active: &Rc<RefCell<bool>>,
    current_url: &Rc<RefCell<Option<String>>>,
) {
    let window = window.clone();
    let splash = splash.clone();
    let splash_active = splash_active.clone();
    let current_url = current_url.clone();

    glib::timeout_add_local_once(STARTUP_SPLASH_MIN_SHOW, move || {
        dismiss_startup_splash(&window, &splash, &splash_active, &current_url);
    });
}

fn dismiss_startup_splash(
    window: &gtk::ApplicationWindow,
    splash: &SplashView,
    splash_active: &Rc<RefCell<bool>>,
    current_url: &Rc<RefCell<Option<String>>>,
) {
    let mut active = splash_active.borrow_mut();
    if !*active {
        return;
    }
    *active = false;
    drop(active);

    stop_splash_animation(splash);
    set_splash_content_fit(splash, gtk::ContentFit::Fill);

    let window = window.clone();
    let splash = splash.clone();
    let current_url = current_url.clone();
    let start = Instant::now();

    glib::timeout_add_local(SPLASH_FRAME_INTERVAL, move || {
        let progress =
            (start.elapsed().as_secs_f64() / STARTUP_SPLASH_POWER_OFF.as_secs_f64()).min(1.0);
        let frame = crt_power_off_frame(&splash, progress);
        apply_power_off_frame(&splash, frame);

        if progress >= 1.0 {
            splash.stage.set_visible(false);
            splash.stage.set_opacity(1.0);
            reset_splash_animation_state(&splash);

            if current_url.borrow().is_none() {
                window.set_visible(false);
            }

            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });
}

fn apply_power_off_frame(splash: &SplashView, frame: PowerOffFrame) {
    set_logo_frame_size(splash, frame.width, frame.height);
    splash.base.set_opacity(frame.line_opacity.clamp(0.0, 1.0));
    splash
        .ghost_a
        .set_opacity(frame.ghost_opacity.clamp(0.0, 1.0));
    splash
        .ghost_b
        .set_opacity((frame.ghost_opacity * 0.82).clamp(0.0, 1.0));
    splash
        .scanlines
        .set_opacity(frame.scanline_opacity.clamp(0.0, 1.0));
    splash
        .stage
        .set_opacity(frame.stage_opacity.clamp(0.0, 1.0));
    set_power_dot_strength(splash, frame.dot_strength);
}

fn crt_power_off_frame(splash: &SplashView, progress: f64) -> PowerOffFrame {
    let t = progress.clamp(0.0, 1.0);

    if t < CRT_VERTICAL_COLLAPSE_PORTION {
        let phase = ease_in_out_cubic(t / CRT_VERTICAL_COLLAPSE_PORTION);
        PowerOffFrame {
            width: splash.logo_width,
            height: lerp(splash.logo_height as f64, CRT_LINE_HEIGHT, phase).round() as i32,
            stage_opacity: 1.0,
            line_opacity: 1.0,
            ghost_opacity: lerp(0.20, 0.10, phase),
            scanline_opacity: lerp(0.42, 0.32, phase),
            dot_strength: 0.0,
        }
    } else {
        let phase = (t - CRT_VERTICAL_COLLAPSE_PORTION) / (1.0 - CRT_VERTICAL_COLLAPSE_PORTION);
        let width_factor = (1.0 - phase.powf(3.3)).clamp(0.0, 1.0);
        let bulge = (1.0 - ((phase - 0.72).abs() / 0.28)).clamp(0.0, 1.0);
        let stage_fade = if phase < 0.58 {
            1.0
        } else {
            1.0 - ease_in_out_cubic((phase - 0.58) / 0.42)
        };
        let line_fade = if phase < 0.45 {
            1.0
        } else {
            1.0 - (ease_in_out_cubic((phase - 0.45) / 0.55) * 0.62)
        };

        PowerOffFrame {
            width: (splash.logo_width as f64 * width_factor).round() as i32,
            height: (CRT_LINE_HEIGHT + (bulge * 11.0)).round() as i32,
            stage_opacity: stage_fade,
            line_opacity: line_fade,
            ghost_opacity: (0.18 * (1.0 - phase)).clamp(0.0, 0.18),
            scanline_opacity: (0.30 * (1.0 - phase * 0.85)).clamp(0.0, 0.30),
            dot_strength: power_dot_curve(phase),
        }
    }
}

fn power_dot_curve(phase: f64) -> f64 {
    if phase < 0.46 {
        0.0
    } else if phase < 0.80 {
        ((phase - 0.46) / 0.34).clamp(0.0, 1.0)
    } else {
        (1.0 - ((phase - 0.80) / 0.20)).clamp(0.0, 1.0)
    }
}

fn set_logo_frame_size(splash: &SplashView, width: i32, height: i32) {
    let width = width.clamp(1, splash.logo_width);
    let height = height.clamp(1, splash.logo_height);

    splash.base.set_size_request(width, height);
    splash.ghost_a.set_size_request(width, height);
    splash.ghost_b.set_size_request(width, height);
    splash.scanlines.set_size_request(width, height);

    let x = (splash.logo_width - width) as f64 / 2.0;
    let y = (splash.logo_height - height) as f64 / 2.0;

    splash.logo_stage.move_(&splash.base, x, y);
    splash.logo_stage.move_(&splash.ghost_a, x, y);
    splash.logo_stage.move_(&splash.ghost_b, x, y);
    splash.logo_stage.move_(&splash.scanlines, x, y);
}

fn stop_splash_animation(splash: &SplashView) {
    if let Some(source_id) = splash.animation_source.borrow_mut().take() {
        source_id.remove();
    }
}

fn reset_splash_animation_state(splash: &SplashView) {
    set_splash_content_fit(splash, gtk::ContentFit::Contain);
    set_logo_frame_size(splash, splash.logo_width, splash.logo_height);
    splash.base.set_opacity(1.0);
    splash.ghost_a.set_opacity(0.0);
    splash.ghost_b.set_opacity(0.0);
    splash.scanlines.set_opacity(0.22);
    set_power_dot_strength(splash, 0.0);
}

fn set_splash_content_fit(splash: &SplashView, fit: gtk::ContentFit) {
    splash.base.set_content_fit(fit);
    splash.ghost_a.set_content_fit(fit);
    splash.ghost_b.set_content_fit(fit);
}

fn set_power_dot_strength(splash: &SplashView, strength: f64) {
    let strength = strength.clamp(0.0, 1.0);
    *splash.power_dot_strength.borrow_mut() = strength;
    splash.power_dot.set_opacity(strength);
    splash.power_dot.queue_draw();
}

fn new_splash_layer(width: i32, height: i32) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_size_request(width, height);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(false);
    picture.set_vexpand(false);
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
    picture.set_can_target(false);
    picture
}

fn new_scanline_layer(width: i32, height: i32) -> gtk::DrawingArea {
    let scanlines = gtk::DrawingArea::new();
    scanlines.set_size_request(width, height);
    scanlines.set_halign(gtk::Align::Fill);
    scanlines.set_valign(gtk::Align::Fill);
    scanlines.set_hexpand(false);
    scanlines.set_vexpand(false);
    scanlines.set_can_target(false);

    scanlines.set_draw_func(|_, cr, width, height| {
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.38);
        let mut y = 0;
        while y < height {
            cr.rectangle(0.0, y as f64, width as f64, 1.0);
            y += SPLASH_SCANLINE_STRIDE;
        }
        let _ = cr.fill();

        cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        let mut y = 1;
        while y < height {
            cr.rectangle(0.0, y as f64, width as f64, 1.0);
            y += SPLASH_SCANLINE_STRIDE * 6;
        }
        let _ = cr.fill();
    });

    scanlines
}

fn new_power_dot_layer(size: i32, strength: Rc<RefCell<f64>>) -> gtk::DrawingArea {
    let dot = gtk::DrawingArea::new();
    dot.set_size_request(size, size);
    dot.set_halign(gtk::Align::Center);
    dot.set_valign(gtk::Align::Center);
    dot.set_can_target(false);

    dot.set_draw_func(move |_, cr, width, height| {
        let level = *strength.borrow();
        if level <= 0.001 {
            return;
        }

        let cx = width as f64 / 2.0;
        let cy = height as f64 / 2.0;
        let radius = (width.min(height) as f64 * 0.08) + level * (width.min(height) as f64 * 0.35);

        cr.set_source_rgba(1.0, 1.0, 1.0, 0.22 * level);
        cr.arc(cx, cy, radius * 1.85, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();

        cr.set_source_rgba(1.0, 0.99, 0.93, 0.95 * level);
        cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.fill();
    });

    dot
}

fn lerp(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress.clamp(0.0, 1.0)
}
