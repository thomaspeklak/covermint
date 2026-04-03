use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{
    cell::RefCell,
    env,
    process::Command,
    rc::Rc,
    time::{Duration, Instant},
};

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
    transition: Transition,
    transition_ms: u32,
    poll_seconds: u32,
    layer: ShellLayer,
    list_monitors: bool,
}

#[derive(Clone, Copy, Debug)]
enum ShellLayer {
    Background,
    Bottom,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtworkSlot {
    Primary,
    Secondary,
}

#[derive(Debug)]
struct MediaState {
    status: PlaybackStatus,
    art_url: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum PlaybackStatus {
    Playing,
    NotPlaying,
}

fn main() -> glib::ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "usage: covermint [--monitor auto|eDP-1] [--player auto|spotify] [--size 420] [--width 520] [--height 420] [--placement bottom-right] [--offset-x 48] [--offset-y 48] [--margin 48] [--border-width 2] [--border-color 'rgba(255,255,255,0.35)'] [--transition fade|flip|none] [--transition-ms 180] [--poll-seconds 2] [--layer background|bottom] [--list-monitors]"
            );
            return glib::ExitCode::FAILURE;
        }
    };

    let app = gtk::Application::builder()
        .application_id("dev.tgz.covermint")
        .build();

    let config = Rc::new(config);
    app.connect_activate(move |app| {
        if config.list_monitors {
            list_monitors();
            app.quit();
            return;
        }

        if !gtk4_layer_shell::is_supported() {
            eprintln!("gtk4-layer-shell is not supported by this compositor/session");
            app.quit();
            return;
        }

        build_ui(app, config.clone());
    });

    app.run_with_args(&["covermint"])
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let mut config = Self {
            monitor_selector: "auto".to_string(),
            player: "auto".to_string(),
            width: 420,
            height: 420,
            placement: Placement::BottomRight,
            offset_x: 48,
            offset_y: 48,
            border_width: 0,
            border_color: "rgba(255,255,255,0.35)".to_string(),
            transition: Transition::Fade,
            transition_ms: 180,
            poll_seconds: 2,
            layer: ShellLayer::Background,
            list_monitors: false,
        };

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
                "--layer" => {
                    config.layer = match next_arg(&mut args, "--layer")?.as_str() {
                        "background" => ShellLayer::Background,
                        "bottom" => ShellLayer::Bottom,
                        value => {
                            return Err(format!(
                                "unsupported --layer value '{value}', expected background or bottom"
                            ));
                        }
                    }
                }
                "--list-monitors" => config.list_monitors = true,
                "--help" | "-h" => return Err("".to_string()),
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(config)
    }
}

impl Transition {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "fade" => Ok(Self::Fade),
            "flip" => Ok(Self::Flip),
            other => Err(format!(
                "unsupported --transition value '{other}', expected one of: none, fade, flip"
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

fn build_ui(app: &gtk::Application, config: Rc<Config>) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("covermint")
        .resizable(false)
        .build();

    window.set_decorated(false);
    window.set_focusable(false);
    window.set_can_focus(false);
    window.set_can_target(false);
    window.set_default_size(config.width, config.height);
    window.add_css_class("covermint-window");

    window.init_layer_shell();
    window.set_namespace(Some("covermint"));
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_layer(match config.layer {
        ShellLayer::Background => Layer::Background,
        ShellLayer::Bottom => Layer::Bottom,
    });
    window.set_exclusive_zone(0);

    let selected_monitor = select_monitor(&config.monitor_selector);
    if let Some(monitor) = selected_monitor.as_ref() {
        window.set_monitor(Some(monitor));
        eprintln!("covermint: using monitor {}", monitor_label(monitor));
    } else {
        eprintln!(
            "covermint: monitor selector '{}' not found, compositor will choose",
            config.monitor_selector
        );
    }

    apply_placement(&window, &config, selected_monitor.as_ref());
    install_styles(&config);

    let primary_picture = new_artwork_picture(&config);
    let secondary_picture = new_artwork_picture(&config);
    secondary_picture.set_opacity(0.0);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&primary_picture));
    overlay.add_overlay(&secondary_picture);

    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class("covermint-artwork");
    frame.set_child(Some(&overlay));

    window.set_child(Some(&frame));
    window.present();
    window.set_visible(false);

    let current_url = Rc::new(RefCell::new(None::<String>));
    let active_slot = Rc::new(RefCell::new(ArtworkSlot::Primary));
    let transition_source = Rc::new(RefCell::new(None::<glib::SourceId>));
    let primary_picture_ref = primary_picture.clone();
    let secondary_picture_ref = secondary_picture.clone();
    let window_ref = window.clone();
    let config_ref = config.clone();

    let refresh = move || match query_player(&config_ref.player) {
        Some(MediaState {
            status: PlaybackStatus::Playing,
            art_url: Some(art_url),
        }) => {
            let needs_reload = current_url
                .borrow()
                .as_ref()
                .map(|current| current != &art_url)
                .unwrap_or(true);

            if needs_reload {
                match download_texture(&art_url) {
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
                        clear_artwork(
                            &primary_picture_ref,
                            &secondary_picture_ref,
                            &active_slot,
                            &transition_source,
                            &config_ref,
                        );
                        *current_url.borrow_mut() = None;
                        window_ref.set_visible(false);
                        return;
                    }
                }
            }

            window_ref.set_visible(true);
        }
        _ => {
            clear_artwork(
                &primary_picture_ref,
                &secondary_picture_ref,
                &active_slot,
                &transition_source,
                &config_ref,
            );
            *current_url.borrow_mut() = None;
            window_ref.set_visible(false);
        }
    };

    refresh();

    glib::timeout_add_seconds_local(config.poll_seconds, move || {
        refresh();
        glib::ControlFlow::Continue
    });
}

fn new_artwork_picture(config: &Config) -> gtk::Picture {
    let picture = gtk::Picture::new();
    picture.set_width_request(config.width);
    picture.set_height_request(config.height);
    picture.set_can_shrink(false);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_halign(gtk::Align::Center);
    picture.set_valign(gtk::Align::Center);
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
    opacity: f64,
}

fn reset_picture_size(picture: &gtk::Picture, width: i32, height: i32) {
    picture.set_width_request(width);
    picture.set_height_request(height);
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

fn flip_width(width: i32, progress: f64) -> i32 {
    ((width as f64 * progress.clamp(0.0, 1.08)).round() as i32).max(1)
}

fn render_picture_frame(
    picture: &gtk::Picture,
    width: i32,
    height: i32,
    transition: Transition,
    frame: TransitionFrame,
) {
    let frame_width = match transition {
        Transition::Flip => flip_width(width, frame.width_progress),
        Transition::None | Transition::Fade => width,
    };
    picture.set_width_request(frame_width);
    picture.set_height_request(height);
    picture.set_opacity(frame.opacity.clamp(0.0, 1.0));
}

fn transition_frames(transition: Transition, progress: f64) -> (TransitionFrame, TransitionFrame) {
    let t = progress.clamp(0.0, 1.0);

    match transition {
        Transition::None => (
            TransitionFrame {
                width_progress: 1.0,
                opacity: 0.0,
            },
            TransitionFrame {
                width_progress: 1.0,
                opacity: 1.0,
            },
        ),
        Transition::Fade => {
            let eased = ease_in_out_cubic(t);
            (
                TransitionFrame {
                    width_progress: 1.0,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
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
                        opacity: 1.0 - (phase * 0.85),
                    },
                    TransitionFrame {
                        width_progress: 0.0,
                        opacity: 0.0,
                    },
                )
            } else {
                let phase = (t - 0.5) / 0.5;
                (
                    TransitionFrame {
                        width_progress: 0.0,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: ease_out_back_subtle(phase),
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
        let (active_picture, inactive_picture) =
            active_picture_pair(primary, secondary, *active_slot.borrow());
        reset_picture_size(&active_picture, config.width, config.height);
        active_picture.set_paintable(Some(texture));
        active_picture.set_opacity(1.0);
        clear_picture(&inactive_picture, config.width, config.height);
        return;
    }

    let current_slot = *active_slot.borrow();
    let next_slot = match current_slot {
        ArtworkSlot::Primary => ArtworkSlot::Secondary,
        ArtworkSlot::Secondary => ArtworkSlot::Primary,
    };
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
    let transition_source = transition_source.clone();
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
            render_picture_frame(
                &to_picture,
                width,
                height,
                transition,
                TransitionFrame {
                    width_progress: 1.0,
                    opacity: 1.0,
                },
            );
            *active_slot.borrow_mut() = next_slot;
            *transition_source.borrow_mut() = None;
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });

    *transition_source.borrow_mut() = Some(source_id);
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

fn apply_placement(
    window: &gtk::ApplicationWindow,
    config: &Config,
    monitor: Option<&gdk::Monitor>,
) {
    reset_window_position(window);

    if let Some(monitor) = monitor {
        let geometry = monitor.geometry();
        let x = axis_offset(
            config.placement.horizontal(),
            geometry.width(),
            config.width,
            config.offset_x,
        );
        let y = axis_offset(
            config.placement.vertical(),
            geometry.height(),
            config.height,
            config.offset_y,
        );

        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Top, true);
        window.set_margin(Edge::Left, x);
        window.set_margin(Edge::Top, y);
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

fn axis_offset(alignment: AxisPlacement, available: i32, size: i32, offset: i32) -> i32 {
    match alignment {
        AxisPlacement::Start => offset,
        AxisPlacement::Center => ((available - size) / 2) + offset,
        AxisPlacement::End => available - size - offset,
    }
}

fn apply_anchor_fallback(window: &gtk::ApplicationWindow, config: &Config) {
    match config.placement {
        Placement::TopLeft => {
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Top, true);
            window.set_margin(Edge::Left, config.offset_x);
            window.set_margin(Edge::Top, config.offset_y);
        }
        Placement::TopRight => {
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Top, true);
            window.set_margin(Edge::Right, config.offset_x);
            window.set_margin(Edge::Top, config.offset_y);
        }
        Placement::BottomLeft => {
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Bottom, true);
            window.set_margin(Edge::Left, config.offset_x);
            window.set_margin(Edge::Bottom, config.offset_y);
        }
        Placement::BottomRight => {
            window.set_anchor(Edge::Right, true);
            window.set_anchor(Edge::Bottom, true);
            window.set_margin(Edge::Right, config.offset_x);
            window.set_margin(Edge::Bottom, config.offset_y);
        }
        placement => {
            eprintln!(
                "covermint: placement '{}' needs monitor geometry; falling back to top-left because the monitor could not be resolved",
                placement.label()
            );
            window.set_anchor(Edge::Left, true);
            window.set_anchor(Edge::Top, true);
            window.set_margin(Edge::Left, config.offset_x);
            window.set_margin(Edge::Top, config.offset_y);
        }
    }
}

fn install_styles(config: &Config) {
    let provider = gtk::CssProvider::new();
    let border_width = config.border_width.max(0);
    let css = format!(
        ".covermint-window {{ background-color: transparent; box-shadow: none; }}\n.covermint-artwork {{ background-color: transparent; box-shadow: none; border-style: solid; border-width: {border_width}px; border-color: {}; }}",
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

fn list_monitors() {
    match gdk::Display::default() {
        Some(display) => {
            let monitors = display.monitors();
            for index in 0..monitors.n_items() {
                if let Some(item) = monitors.item(index) {
                    if let Ok(monitor) = item.downcast::<gdk::Monitor>() {
                        let geometry = monitor.geometry();
                        println!(
                            "#{index}: {} [{}x{}+{}+{} scale={}]",
                            monitor_label(&monitor),
                            geometry.width(),
                            geometry.height(),
                            geometry.x(),
                            geometry.y(),
                            monitor.scale_factor()
                        );
                    }
                }
            }
        }
        None => eprintln!("covermint: no GTK display available"),
    }
}

fn select_monitor(selector: &str) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    let mut all = Vec::new();

    for index in 0..monitors.n_items() {
        if let Some(item) = monitors.item(index) {
            if let Ok(monitor) = item.downcast::<gdk::Monitor>() {
                all.push(monitor);
            }
        }
    }

    if all.is_empty() {
        return None;
    }

    if selector.eq_ignore_ascii_case("auto") {
        return all
            .iter()
            .find(|monitor| {
                monitor
                    .connector()
                    .map(|connector| is_internal_connector(connector.as_str()))
                    .unwrap_or(false)
            })
            .cloned()
            .or_else(|| all.first().cloned());
    }

    let needle = selector.to_ascii_lowercase();
    all.into_iter().find(|monitor| {
        [
            monitor.connector().map(|v| v.to_string()),
            monitor.manufacturer().map(|v| v.to_string()),
            monitor.model().map(|v| v.to_string()),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.to_ascii_lowercase().contains(&needle))
    })
}

fn is_internal_connector(connector: &str) -> bool {
    let lower = connector.to_ascii_lowercase();
    lower.starts_with("edp") || lower.starts_with("lvds") || lower.starts_with("dsi")
}

fn monitor_label(monitor: &gdk::Monitor) -> String {
    let connector = monitor.connector().map(|value| value.to_string());
    let manufacturer = monitor.manufacturer().map(|value| value.to_string());
    let model = monitor.model().map(|value| value.to_string());

    [
        connector,
        manufacturer.zip(model).map(|(a, b)| format!("{a} {b}")),
    ]
    .into_iter()
    .flatten()
    .next()
    .unwrap_or_else(|| "unknown monitor".to_string())
}

fn query_player(player: &str) -> Option<MediaState> {
    let status = run_playerctl(player, &["status"])?;
    let status = if status.trim() == "Playing" {
        PlaybackStatus::Playing
    } else {
        PlaybackStatus::NotPlaying
    };

    let art_url = run_playerctl(player, &["metadata", "mpris:artUrl"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Some(MediaState { status, art_url })
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

fn download_texture(url: &str) -> Option<gdk::Texture> {
    let response = reqwest::blocking::get(url).ok()?.error_for_status().ok()?;
    let bytes = response.bytes().ok()?;
    let bytes = glib::Bytes::from_owned(bytes.to_vec());
    gdk::Texture::from_bytes(&bytes).ok()
}
