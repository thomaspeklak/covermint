use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{
    cell::RefCell,
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    process::Command,
    rc::Rc,
    time::{Duration, Instant, SystemTime},
};

const USAGE: &str = "usage: covermint [--monitor auto|internal|external|0|#0|eDP-1] [--player auto|<name>] [--size 420] [--width 520] [--height 420] [--placement bottom-right] [--offset-x 48] [--offset-y 48] [--margin 48] [--border-width 2] [--border-color 'rgba(255,255,255,0.35)'] [--corner-radius 18] [--opacity 0.92] [--transition fade|flip|hinge|none] [--transition-ms 180] [--poll-seconds 2] [--show-paused] [--no-cache] [--cache-max-files 128] [--cache-max-mb 256] [--layer background|bottom] [--list-monitors] [--list-players] [--help]";
const SPLASH_LOGO: &[u8] = include_bytes!("../assets/branding/covermint-logo-grunge.png");

#[derive(Clone, Debug)]
struct Config {
    monitor_selector: String,
    player: String,
    width: i32,
    height: i32,
    placement: Placement,
    offset_x: i32,
    offset_y: i32,
    border_width: i32,
    border_color: String,
    corner_radius: i32,
    opacity: f64,
    transition: Transition,
    transition_ms: u32,
    poll_seconds: u32,
    show_paused: bool,
    cache_enabled: bool,
    cache_max_files: usize,
    cache_max_bytes: u64,
    layer: ShellLayer,
}

#[derive(Clone, Debug)]
enum StartupAction {
    Help,
    ListMonitors,
    ListPlayers,
    Run(Config),
}

#[derive(Clone, Copy, Debug)]
enum ShellLayer {
    Background,
    Bottom,
}

impl ShellLayer {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "background" => Ok(Self::Background),
            "bottom" => Ok(Self::Bottom),
            other => Err(format!(
                "unsupported --layer value '{other}', expected background or bottom"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Placement {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Clone, Copy, Debug)]
enum AxisPlacement {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transition {
    None,
    Fade,
    Flip,
    Hinge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtworkSlot {
    Primary,
    Secondary,
}

impl ArtworkSlot {
    fn other(self) -> Self {
        match self {
            Self::Primary => Self::Secondary,
            Self::Secondary => Self::Primary,
        }
    }
}

#[derive(Debug)]
struct MediaState {
    status: PlaybackStatus,
    art_url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaybackStatus {
    Playing,
    Paused,
    NotPlaying,
}

impl PlaybackStatus {
    fn should_show_artwork(self, show_paused: bool) -> bool {
        self == Self::Playing || (show_paused && self == Self::Paused)
    }

    fn auto_select_rank(self) -> u8 {
        match self {
            Self::Playing => 2,
            Self::Paused => 1,
            Self::NotPlaying => 0,
        }
    }
}

fn main() -> glib::ExitCode {
    let action = match StartupAction::from_env() {
        Ok(action) => action,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("{USAGE}");
            return glib::ExitCode::FAILURE;
        }
    };

    if matches!(&action, StartupAction::Help) {
        println!("{USAGE}");
        return glib::ExitCode::SUCCESS;
    }

    let app = gtk::Application::builder()
        .application_id("dev.tgz.covermint")
        .build();

    app.connect_activate(move |app| match &action {
        StartupAction::Help => app.quit(),
        StartupAction::ListMonitors => {
            list_monitors();
            app.quit();
        }
        StartupAction::ListPlayers => {
            list_players();
            app.quit();
        }
        StartupAction::Run(config) => {
            if !gtk4_layer_shell::is_supported() {
                eprintln!("gtk4-layer-shell is not supported by this compositor/session");
                app.quit();
                return;
            }

            build_ui(app, Rc::new(config.clone()));
        }
    });

    app.run_with_args(&["covermint"])
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor_selector: "auto".to_string(),
            player: "auto".to_string(),
            width: 420,
            height: 420,
            placement: Placement::BottomRight,
            offset_x: 48,
            offset_y: 48,
            border_width: 0,
            border_color: "rgba(255,255,255,0.35)".to_string(),
            corner_radius: 0,
            opacity: 1.0,
            transition: Transition::Fade,
            transition_ms: 180,
            poll_seconds: 2,
            show_paused: false,
            cache_enabled: true,
            cache_max_files: 128,
            cache_max_bytes: 256 * 1024 * 1024,
            layer: ShellLayer::Background,
        }
    }
}

impl StartupAction {
    fn from_env() -> Result<Self, String> {
        let mut config = Config::default();
        let mut list_monitors = false;
        let mut list_players = false;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--monitor" => config.monitor_selector = next_arg(&mut args, "--monitor")?,
                "--player" => config.player = next_arg(&mut args, "--player")?,
                "--size" => {
                    let size = parse_i32(next_arg(&mut args, "--size")?, "--size")?;
                    config.width = size;
                    config.height = size;
                }
                "--width" => config.width = parse_i32(next_arg(&mut args, "--width")?, "--width")?,
                "--height" => {
                    config.height = parse_i32(next_arg(&mut args, "--height")?, "--height")?
                }
                "--placement" => {
                    config.placement = Placement::parse(&next_arg(&mut args, "--placement")?)?
                }
                "--offset-x" => {
                    config.offset_x = parse_i32(next_arg(&mut args, "--offset-x")?, "--offset-x")?
                }
                "--offset-y" => {
                    config.offset_y = parse_i32(next_arg(&mut args, "--offset-y")?, "--offset-y")?
                }
                "--margin" => {
                    let margin = parse_i32(next_arg(&mut args, "--margin")?, "--margin")?;
                    config.offset_x = margin;
                    config.offset_y = margin;
                }
                "--border-width" => {
                    config.border_width =
                        parse_i32(next_arg(&mut args, "--border-width")?, "--border-width")?
                }
                "--border-color" => config.border_color = next_arg(&mut args, "--border-color")?,
                "--corner-radius" => {
                    config.corner_radius =
                        parse_i32(next_arg(&mut args, "--corner-radius")?, "--corner-radius")?
                }
                "--opacity" => config.opacity = parse_opacity(next_arg(&mut args, "--opacity")?)?,
                "--transition" => {
                    config.transition = Transition::parse(&next_arg(&mut args, "--transition")?)?
                }
                "--transition-ms" => {
                    config.transition_ms =
                        parse_u32(next_arg(&mut args, "--transition-ms")?, "--transition-ms")?
                }
                "--poll-seconds" => {
                    config.poll_seconds =
                        parse_u32(next_arg(&mut args, "--poll-seconds")?, "--poll-seconds")?
                }
                "--show-paused" => config.show_paused = true,
                "--no-cache" => config.cache_enabled = false,
                "--cache-max-files" => {
                    config.cache_max_files = parse_usize(
                        next_arg(&mut args, "--cache-max-files")?,
                        "--cache-max-files",
                    )?
                }
                "--cache-max-mb" => {
                    config.cache_max_bytes =
                        parse_u64(next_arg(&mut args, "--cache-max-mb")?, "--cache-max-mb")?
                            .saturating_mul(1024 * 1024)
                }
                "--layer" => config.layer = ShellLayer::parse(&next_arg(&mut args, "--layer")?)?,
                "--list-monitors" => list_monitors = true,
                "--list-players" => list_players = true,
                "--help" | "-h" => return Ok(Self::Help),
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        if list_monitors {
            return Ok(Self::ListMonitors);
        }

        if list_players {
            return Ok(Self::ListPlayers);
        }

        Ok(Self::Run(config))
    }
}

impl Transition {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "fade" => Ok(Self::Fade),
            "flip" => Ok(Self::Flip),
            "hinge" => Ok(Self::Hinge),
            other => Err(format!(
                "unsupported --transition value '{other}', expected one of: none, fade, flip, hinge"
            )),
        }
    }
}

impl Placement {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "top-left" | "tl" => Ok(Self::TopLeft),
            "top" | "top-center" | "tc" => Ok(Self::Top),
            "top-right" | "tr" => Ok(Self::TopRight),
            "left" | "center-left" | "cl" => Ok(Self::Left),
            "center" | "middle" => Ok(Self::Center),
            "right" | "center-right" | "cr" => Ok(Self::Right),
            "bottom-left" | "bl" => Ok(Self::BottomLeft),
            "bottom" | "bottom-center" | "bc" => Ok(Self::Bottom),
            "bottom-right" | "br" => Ok(Self::BottomRight),
            other => Err(format!(
                "unsupported --placement value '{other}', expected one of: top-left, top, top-right, left, center, right, bottom-left, bottom, bottom-right"
            )),
        }
    }

    fn horizontal(self) -> AxisPlacement {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => AxisPlacement::Start,
            Self::Top | Self::Center | Self::Bottom => AxisPlacement::Center,
            Self::TopRight | Self::Right | Self::BottomRight => AxisPlacement::End,
        }
    }

    fn vertical(self) -> AxisPlacement {
        match self {
            Self::TopLeft | Self::Top | Self::TopRight => AxisPlacement::Start,
            Self::Left | Self::Center | Self::Right => AxisPlacement::Center,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => AxisPlacement::End,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::Top => "top",
            Self::TopRight => "top-right",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::BottomLeft => "bottom-left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom-right",
        }
    }

    fn fallback_anchor_edges(self) -> Option<(Edge, Edge)> {
        match self {
            Self::TopLeft => Some((Edge::Left, Edge::Top)),
            Self::TopRight => Some((Edge::Right, Edge::Top)),
            Self::BottomLeft => Some((Edge::Left, Edge::Bottom)),
            Self::BottomRight => Some((Edge::Right, Edge::Bottom)),
            Self::Top | Self::Left | Self::Center | Self::Right | Self::Bottom => None,
        }
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_i32(value: String, flag: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .map_err(|_| format!("invalid integer for {flag}: {value}"))
}

fn parse_u32(value: String, flag: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("invalid integer for {flag}: {value}"))
}

fn parse_u64(value: String, flag: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid integer for {flag}: {value}"))
}

fn parse_usize(value: String, flag: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|_| format!("invalid integer for {flag}: {value}"))
}

fn parse_opacity(value: String) -> Result<f64, String> {
    let opacity = value
        .parse::<f64>()
        .map_err(|_| format!("invalid number for --opacity: {value}"))?;

    if !(0.0..=1.0).contains(&opacity) {
        return Err(format!(
            "unsupported --opacity value '{value}', expected a number between 0.0 and 1.0"
        ));
    }

    Ok(opacity)
}

fn build_ui(app: &gtk::Application, config: Rc<Config>) {
    let (window_width, window_height) = artwork_window_size(&config);
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

    let primary_picture = new_artwork_picture(&config);
    let secondary_picture = new_artwork_picture(&config);
    let splash_picture = new_splash_picture(&config);
    secondary_picture.set_opacity(0.0);

    let splash_enabled = if let Some(texture) = load_texture(SPLASH_LOGO.to_vec()) {
        splash_picture.set_paintable(Some(&texture));
        splash_picture.set_visible(true);
        true
    } else {
        eprintln!("covermint: failed to load embedded splash logo");
        false
    };

    let overlay = gtk::Overlay::new();
    overlay.set_size_request(config.width, config.height);
    overlay.set_halign(gtk::Align::Fill);
    overlay.set_valign(gtk::Align::Fill);
    overlay.set_child(Some(&primary_picture));
    overlay.add_overlay(&secondary_picture);
    overlay.add_overlay(&splash_picture);

    let artwork_stage = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_stage.add_css_class("covermint-artwork-stage");
    artwork_stage.set_size_request(config.width, config.height);
    artwork_stage.set_halign(gtk::Align::Center);
    artwork_stage.set_valign(gtk::Align::Center);
    artwork_stage.append(&overlay);

    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("covermint-artwork");
    frame.set_size_request(window_width, window_height);
    frame.set_halign(gtk::Align::Fill);
    frame.set_valign(gtk::Align::Fill);
    frame.set_opacity(config.opacity);
    frame.append(&artwork_stage);

    window.set_child(Some(&frame));
    window.present();
    window.set_visible(splash_enabled);

    let current_url = Rc::new(RefCell::new(None::<String>));
    let active_slot = Rc::new(RefCell::new(ArtworkSlot::Primary));
    let transition_source = Rc::new(RefCell::new(None::<glib::SourceId>));
    let splash_active = Rc::new(RefCell::new(splash_enabled));
    let primary_picture_ref = primary_picture.clone();
    let secondary_picture_ref = secondary_picture.clone();
    let splash_picture_ref = splash_picture.clone();
    let window_ref = window.clone();
    let config_ref = config.clone();
    let monitor_status_ref = monitor_status.clone();
    let splash_active_ref = splash_active.clone();

    let refresh = move || {
        sync_window_target(&window_ref, &config_ref, &monitor_status_ref);

        let handle_empty_state = || {
            if *splash_active_ref.borrow() {
                finish_startup_splash(&splash_picture_ref, &splash_active_ref);
            }

            clear_artwork_and_hide(
                &window_ref,
                &current_url,
                &primary_picture_ref,
                &secondary_picture_ref,
                &active_slot,
                &transition_source,
                &config_ref,
            );
        };

        match query_player(&config_ref.player) {
            Some(MediaState {
                status,
                art_url: Some(art_url),
            }) if status.should_show_artwork(config_ref.show_paused) => {
                let needs_reload = current_url
                    .borrow()
                    .as_ref()
                    .map(|current| current != &art_url)
                    .unwrap_or(true);

                if needs_reload {
                    match download_texture(&art_url, &config_ref) {
                        Some(texture) => {
                            let has_existing_art = current_url.borrow().is_some();
                            set_artwork_texture(
                                &primary_picture_ref,
                                &secondary_picture_ref,
                                &active_slot,
                                &transition_source,
                                &config_ref,
                                &texture,
                                has_existing_art,
                            );
                            *current_url.borrow_mut() = Some(art_url);
                        }
                        None => {
                            eprintln!("covermint: failed to download artwork");
                            handle_empty_state();
                            return;
                        }
                    }
                }

                if *splash_active_ref.borrow() {
                    finish_startup_splash(&splash_picture_ref, &splash_active_ref);
                }

                window_ref.set_visible(true);
            }
            _ => handle_empty_state(),
        }
    };

    let initial_refresh = refresh.clone();
    glib::idle_add_local_once(move || {
        initial_refresh();
    });

    glib::timeout_add_seconds_local(config.poll_seconds, move || {
        refresh();
        glib::ControlFlow::Continue
    });
}

fn new_artwork_picture(config: &Config) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_size_request(config.width, config.height);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
    picture
}

fn new_splash_picture(config: &Config) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_size_request(config.width, config.height);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
    picture.set_visible(false);
    picture
}

fn run_playerctl<'a>(player: &'a str, args: &[&'a str]) -> Option<String> {
    let mut command_args = Vec::with_capacity(args.len() + 2);
    if !player.eq_ignore_ascii_case("auto") {
        command_args.extend(["-p", player]);
    }
    command_args.extend(args.iter().copied());
    run_command("playerctl", &command_args)
}

fn active_picture_pair(
    primary: &gtk::Picture,
    secondary: &gtk::Picture,
    slot: ArtworkSlot,
) -> (gtk::Picture, gtk::Picture) {
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

#[derive(Clone, Copy)]
struct TransitionFrame {
    width_progress: f64,
    height_progress: f64,
    opacity: f64,
}

fn reset_picture_size(picture: &gtk::Picture, width: i32, height: i32) {
    picture.set_size_request(width, height);
}

fn reset_picture_layout(picture: &gtk::Picture) {
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
}

fn artwork_window_size(config: &Config) -> (i32, i32) {
    let border = config.border_width.max(0) * 2;
    (config.width + border, config.height + border)
}

fn ease_in_out_cubic(progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - ((-2.0 * t + 2.0).powi(3) / 2.0)
    }
}

fn ease_out_back_subtle(progress: f64) -> f64 {
    let t = progress.clamp(0.0, 1.0);
    let overshoot = 0.6;
    let c3 = overshoot + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + overshoot * (t - 1.0).powi(2)
}

fn scaled_frame_size(size: i32, progress: f64, max_progress: f64) -> i32 {
    ((size as f64 * progress.clamp(0.0, max_progress)).round() as i32).max(1)
}

fn render_picture_frame(
    picture: &gtk::Picture,
    width: i32,
    height: i32,
    transition: Transition,
    frame: TransitionFrame,
) {
    let (frame_width, frame_height, halign, valign) = match transition {
        Transition::None | Transition::Fade => (width, height, gtk::Align::Fill, gtk::Align::Fill),
        Transition::Flip => (
            scaled_frame_size(width, frame.width_progress, 1.08),
            height,
            gtk::Align::Center,
            gtk::Align::Center,
        ),
        Transition::Hinge => (
            scaled_frame_size(width, frame.width_progress, 1.04),
            scaled_frame_size(height, frame.height_progress, 1.04),
            gtk::Align::Center,
            gtk::Align::Start,
        ),
    };

    picture.set_halign(halign);
    picture.set_valign(valign);
    picture.set_size_request(frame_width, frame_height);
    picture.set_opacity(frame.opacity.clamp(0.0, 1.0));
}

fn transition_frames(transition: Transition, progress: f64) -> (TransitionFrame, TransitionFrame) {
    let t = progress.clamp(0.0, 1.0);

    match transition {
        Transition::None => (
            TransitionFrame {
                width_progress: 1.0,
                height_progress: 1.0,
                opacity: 0.0,
            },
            TransitionFrame {
                width_progress: 1.0,
                height_progress: 1.0,
                opacity: 1.0,
            },
        ),
        Transition::Fade => {
            let eased = ease_in_out_cubic(t);
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    opacity: eased,
                },
            )
        }
        Transition::Flip => {
            if t < 0.5 {
                let phase = ease_in_out_cubic(t / 0.5);
                (
                    TransitionFrame {
                        width_progress: 1.0 - phase,
                        height_progress: 1.0,
                        opacity: 1.0 - (phase * 0.85),
                    },
                    TransitionFrame {
                        width_progress: 0.0,
                        height_progress: 1.0,
                        opacity: 0.0,
                    },
                )
            } else {
                let phase = (t - 0.5) / 0.5;
                (
                    TransitionFrame {
                        width_progress: 0.0,
                        height_progress: 1.0,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: ease_out_back_subtle(phase),
                        height_progress: 1.0,
                        opacity: ease_in_out_cubic(phase),
                    },
                )
            }
        }
        Transition::Hinge => {
            if t < 0.5 {
                let phase = ease_in_out_cubic(t / 0.5);
                (
                    TransitionFrame {
                        width_progress: 1.0 - (phase * 0.72),
                        height_progress: 1.0 - (phase * 0.16),
                        opacity: 1.0 - (phase * 0.9),
                    },
                    TransitionFrame {
                        width_progress: 0.28,
                        height_progress: 0.84,
                        opacity: 0.0,
                    },
                )
            } else {
                let phase = (t - 0.5) / 0.5;
                let spring = ease_out_back_subtle(phase);
                (
                    TransitionFrame {
                        width_progress: 0.28,
                        height_progress: 0.84,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: 0.28 + (spring * 0.72),
                        height_progress: 0.84 + (spring * 0.16),
                        opacity: ease_in_out_cubic(phase),
                    },
                )
            }
        }
    }
}

fn clear_picture(picture: &gtk::Picture, width: i32, height: i32) {
    picture.set_paintable(Option::<&gdk::Texture>::None);
    picture.set_opacity(0.0);
    reset_picture_size(picture, width, height);
    reset_picture_layout(picture);
}

fn set_artwork_texture_immediate(
    primary: &gtk::Picture,
    secondary: &gtk::Picture,
    active_slot: ArtworkSlot,
    config: &Config,
    texture: &gdk::Texture,
) {
    let (active_picture, inactive_picture) = active_picture_pair(primary, secondary, active_slot);
    reset_picture_size(&active_picture, config.width, config.height);
    reset_picture_layout(&active_picture);
    active_picture.set_paintable(Some(texture));
    active_picture.set_opacity(1.0);
    clear_picture(&inactive_picture, config.width, config.height);
}

fn animate_artwork_transition(
    primary: &gtk::Picture,
    secondary: &gtk::Picture,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
    texture: &gdk::Texture,
) {
    let current_slot = *active_slot.borrow();
    let next_slot = current_slot.other();
    let (from_picture, to_picture) = active_picture_pair(primary, secondary, current_slot);

    to_picture.set_paintable(Some(texture));
    let (from_start, to_start) = transition_frames(config.transition, 0.0);
    render_picture_frame(
        &from_picture,
        config.width,
        config.height,
        config.transition,
        from_start,
    );
    render_picture_frame(
        &to_picture,
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

    let source_id = glib::timeout_add_local(Duration::from_millis(16), move || {
        let progress = (start.elapsed().as_secs_f64() / duration.as_secs_f64()).min(1.0);
        let (from_frame, to_frame) = transition_frames(transition, progress);
        render_picture_frame(&from_picture, width, height, transition, from_frame);
        render_picture_frame(&to_picture, width, height, transition, to_frame);

        if progress >= 1.0 {
            clear_picture(&from_picture, width, height);
            reset_picture_size(&to_picture, width, height);
            reset_picture_layout(&to_picture);
            to_picture.set_opacity(1.0);
            *active_slot.borrow_mut() = next_slot;
            *transition_source_for_closure.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });

    *transition_source.borrow_mut() = Some(source_id);
}

fn set_artwork_texture(
    primary: &gtk::Picture,
    secondary: &gtk::Picture,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
    texture: &gdk::Texture,
    animate: bool,
) {
    stop_transition(transition_source);

    if !animate || config.transition == Transition::None || config.transition_ms == 0 {
        set_artwork_texture_immediate(primary, secondary, *active_slot.borrow(), config, texture);
        return;
    }

    animate_artwork_transition(
        primary,
        secondary,
        active_slot,
        transition_source,
        config,
        texture,
    );
}

fn clear_artwork(
    primary: &gtk::Picture,
    secondary: &gtk::Picture,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
) {
    stop_transition(transition_source);

    clear_picture(primary, config.width, config.height);
    clear_picture(secondary, config.width, config.height);
    primary.set_opacity(1.0);
    *active_slot.borrow_mut() = ArtworkSlot::Primary;
}

fn clear_artwork_and_hide(
    window: &gtk::ApplicationWindow,
    current_url: &Rc<RefCell<Option<String>>>,
    primary: &gtk::Picture,
    secondary: &gtk::Picture,
    active_slot: &Rc<RefCell<ArtworkSlot>>,
    transition_source: &Rc<RefCell<Option<glib::SourceId>>>,
    config: &Config,
) {
    clear_artwork(primary, secondary, active_slot, transition_source, config);
    *current_url.borrow_mut() = None;
    window.set_visible(false);
}

fn finish_startup_splash(splash_picture: &gtk::Picture, splash_active: &Rc<RefCell<bool>>) {
    splash_picture.set_visible(false);
    splash_picture.set_opacity(1.0);
    *splash_active.borrow_mut() = false;
}

fn sync_window_target(
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
        let (window_width, window_height) = artwork_window_size(config);
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
    let (horizontal_edge, vertical_edge) = match config.placement.fallback_anchor_edges() {
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

fn install_styles(config: &Config) {
    let provider = gtk::CssProvider::new();
    let border_width = config.border_width.max(0);
    let corner_radius = config.corner_radius.max(0);
    let inner_radius = (corner_radius - border_width).max(0);
    let css = format!(
        ".covermint-window {{ background-color: transparent; box-shadow: none; border-radius: {corner_radius}px; }}\n.covermint-artwork {{ background-color: transparent; box-shadow: none; border-style: solid; border-width: {border_width}px; border-color: {}; border-radius: {corner_radius}px; }}\n.covermint-artwork-stage {{ background-color: transparent; box-shadow: none; border-radius: {inner_radius}px; }}",
        config.border_color
    );
    provider.load_from_data(&css);

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn collect_monitors(display: &gdk::Display) -> Vec<gdk::Monitor> {
    let monitors = display.monitors();
    let mut all = Vec::new();

    for index in 0..monitors.n_items() {
        if let Some(item) = monitors.item(index) {
            if let Ok(monitor) = item.downcast::<gdk::Monitor>() {
                all.push(monitor);
            }
        }
    }

    all
}

fn list_monitors() {
    match gdk::Display::default() {
        Some(display) => {
            for (index, monitor) in collect_monitors(&display).into_iter().enumerate() {
                let geometry = monitor.geometry();
                let role = if monitor_is_internal(&monitor) {
                    "internal"
                } else {
                    "external"
                };
                println!(
                    "#{index}: {} ({role}) [{}x{}+{}+{} scale={}]",
                    monitor_label(&monitor),
                    geometry.width(),
                    geometry.height(),
                    geometry.x(),
                    geometry.y(),
                    monitor.scale_factor()
                );
            }
        }
        None => eprintln!("covermint: no GTK display available"),
    }
}

fn player_names() -> Vec<String> {
    run_command("playerctl", &["-l"])
        .map(|players| {
            players
                .lines()
                .map(str::trim)
                .filter(|player| !player.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn list_players() {
    let players = player_names();
    if players.is_empty() {
        eprintln!("covermint: no MPRIS players reported by playerctl");
        return;
    }

    println!("{}", players.join("\n"));
}

fn select_monitor(selector: &str) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let all = collect_monitors(&display);

    if all.is_empty() {
        return None;
    }

    if selector.eq_ignore_ascii_case("auto") || selector.eq_ignore_ascii_case("internal") {
        let internal = all.iter().find(|monitor| monitor_is_internal(monitor));

        if selector.eq_ignore_ascii_case("internal") {
            return internal.cloned();
        }

        return internal.cloned().or_else(|| all.first().cloned());
    }

    if selector.eq_ignore_ascii_case("external") {
        return all
            .iter()
            .find(|monitor| !monitor_is_internal(monitor))
            .cloned()
            .or_else(|| all.first().cloned());
    }

    let selector = selector.trim();
    if let Some(index) = selector
        .strip_prefix('#')
        .unwrap_or(selector)
        .parse::<usize>()
        .ok()
    {
        return all.get(index).cloned();
    }

    let needle = selector.to_ascii_lowercase();
    all.into_iter().find(|monitor| {
        monitor_search_terms(monitor)
            .into_iter()
            .any(|value| value.to_ascii_lowercase().contains(&needle))
    })
}

fn monitor_parts(monitor: &gdk::Monitor) -> (Option<String>, Option<String>, Option<String>) {
    (
        monitor.connector().map(|value| value.to_string()),
        monitor.manufacturer().map(|value| value.to_string()),
        monitor.model().map(|value| value.to_string()),
    )
}

fn monitor_is_internal(monitor: &gdk::Monitor) -> bool {
    let (connector, _, _) = monitor_parts(monitor);
    connector
        .as_deref()
        .map(is_internal_connector)
        .unwrap_or(false)
}

fn is_internal_connector(connector: &str) -> bool {
    let lower = connector.to_ascii_lowercase();
    lower.starts_with("edp") || lower.starts_with("lvds") || lower.starts_with("dsi")
}

fn monitor_description(monitor: &gdk::Monitor) -> Option<String> {
    let (_, manufacturer, model) = monitor_parts(monitor);
    match (manufacturer, model) {
        (Some(manufacturer), Some(model)) => Some(format!("{manufacturer} {model}")),
        (Some(manufacturer), None) => Some(manufacturer),
        (None, Some(model)) => Some(model),
        (None, None) => None,
    }
}

fn monitor_search_terms(monitor: &gdk::Monitor) -> Vec<String> {
    let (connector, manufacturer, model) = monitor_parts(monitor);
    let description = monitor_description(monitor);

    [connector, description, manufacturer, model]
        .into_iter()
        .flatten()
        .collect()
}

fn monitor_label(monitor: &gdk::Monitor) -> String {
    let (connector, _, _) = monitor_parts(monitor);
    match (connector, monitor_description(monitor)) {
        (Some(connector), Some(description)) => format!("{connector} — {description}"),
        (Some(connector), None) => connector,
        (None, Some(description)) => description,
        (None, None) => "unknown monitor".to_string(),
    }
}

fn query_named_player(player: &str) -> Option<MediaState> {
    let status = run_playerctl(player, &["status"])?;
    let status = match status.trim() {
        "Playing" => PlaybackStatus::Playing,
        "Paused" => PlaybackStatus::Paused,
        _ => PlaybackStatus::NotPlaying,
    };

    let art_url = run_playerctl(player, &["metadata", "mpris:artUrl"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Some(MediaState { status, art_url })
}

fn query_player(player: &str) -> Option<MediaState> {
    if !player.eq_ignore_ascii_case("auto") {
        return query_named_player(player);
    }

    let mut best_match = None;

    for player_name in player_names() {
        let Some(state) = query_named_player(&player_name) else {
            continue;
        };
        let score = (state.status.auto_select_rank(), state.art_url.is_some());

        if best_match
            .as_ref()
            .map(|(best_score, _): &((u8, bool), MediaState)| score > *best_score)
            .unwrap_or(true)
        {
            best_match = Some((score, state));
        }
    }

    best_match
        .map(|(_, state)| state)
        .or_else(|| query_named_player(player))
}

fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|stdout| stdout.trim().to_string())
}

fn load_texture(bytes: Vec<u8>) -> Option<gdk::Texture> {
    let bytes = glib::Bytes::from_owned(bytes);
    gdk::Texture::from_bytes(&bytes).ok()
}

fn cache_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("covermint").join("artwork"))
}

fn cache_path(url: &str) -> Option<PathBuf> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);

    let dir = cache_dir()?;
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{:016x}.img", hasher.finish())))
}

fn trim_cache(dir: &PathBuf, max_files: usize, max_bytes: u64) {
    const MAX_CACHE_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30);

    let now = SystemTime::now();
    let mut entries = Vec::new();
    let mut total_bytes = 0_u64;

    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        let modified = metadata.modified().ok();
        let is_stale = modified
            .and_then(|time| now.duration_since(time).ok())
            .map(|age| age > MAX_CACHE_AGE)
            .unwrap_or(false);

        if is_stale {
            let _ = fs::remove_file(entry.path());
            continue;
        }

        let size = metadata.len();
        total_bytes = total_bytes.saturating_add(size);
        entries.push((modified, entry.path(), size));
    }

    entries.sort_by_key(|(modified, _, _)| *modified);

    while entries.len() > max_files || total_bytes > max_bytes {
        let Some((_, path, size)) = entries.first().cloned() else {
            break;
        };
        entries.remove(0);
        total_bytes = total_bytes.saturating_sub(size);
        let _ = fs::remove_file(path);
    }
}

fn artwork_bytes(url: &str) -> Option<Vec<u8>> {
    let parsed = reqwest::Url::parse(url).ok()?;
    match parsed.scheme() {
        "file" => fs::read(parsed.to_file_path().ok()?).ok(),
        "http" | "https" => {
            let response = reqwest::blocking::get(url).ok()?.error_for_status().ok()?;
            Some(response.bytes().ok()?.to_vec())
        }
        _ => None,
    }
}

fn download_texture(url: &str, config: &Config) -> Option<gdk::Texture> {
    if config.cache_enabled {
        if let Some(path) = cache_path(url) {
            if let Ok(bytes) = fs::read(&path) {
                if let Some(texture) = load_texture(bytes.clone()) {
                    let _ = fs::write(&path, &bytes);
                    return Some(texture);
                }
                let _ = fs::remove_file(&path);
            }

            let bytes = artwork_bytes(url)?;
            let _ = fs::write(&path, &bytes);
            if let Some(dir) = path.parent() {
                trim_cache(
                    &dir.to_path_buf(),
                    config.cache_max_files,
                    config.cache_max_bytes,
                );
            }
            return load_texture(bytes);
        }
    }

    load_texture(artwork_bytes(url)?)
}
