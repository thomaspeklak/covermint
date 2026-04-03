use gtk::{gdk, glib, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::{cell::RefCell, env, process::Command, rc::Rc};

#[derive(Clone, Debug)]
struct Config {
    monitor_selector: String,
    player: String,
    size: i32,
    margin: i32,
    poll_seconds: u32,
    layer: ShellLayer,
    list_monitors: bool,
}

#[derive(Clone, Copy, Debug)]
enum ShellLayer {
    Background,
    Bottom,
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
            eprintln!("usage: covermint [--monitor auto|eDP-1] [--player spotify] [--size 420] [--margin 48] [--poll-seconds 2] [--layer background|bottom] [--list-monitors]");
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
            player: "spotify".to_string(),
            size: 420,
            margin: 48,
            poll_seconds: 2,
            layer: ShellLayer::Background,
            list_monitors: false,
        };

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--monitor" => config.monitor_selector = next_arg(&mut args, "--monitor")?,
                "--player" => config.player = next_arg(&mut args, "--player")?,
                "--size" => config.size = parse_i32(next_arg(&mut args, "--size")?, "--size")?,
                "--margin" => {
                    config.margin = parse_i32(next_arg(&mut args, "--margin")?, "--margin")?
                }
                "--poll-seconds" => {
                    config.poll_seconds = parse_u32(
                        next_arg(&mut args, "--poll-seconds")?,
                        "--poll-seconds",
                    )?
                }
                "--layer" => {
                    config.layer = match next_arg(&mut args, "--layer")?.as_str() {
                        "background" => ShellLayer::Background,
                        "bottom" => ShellLayer::Bottom,
                        value => {
                            return Err(format!(
                                "unsupported --layer value '{value}', expected background or bottom"
                            ))
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
    window.set_default_size(config.size, config.size);

    window.init_layer_shell();
    window.set_namespace(Some("covermint"));
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_layer(match config.layer {
        ShellLayer::Background => Layer::Background,
        ShellLayer::Bottom => Layer::Bottom,
    });
    window.set_exclusive_zone(0);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Right, config.margin);
    window.set_margin(Edge::Bottom, config.margin);

    if let Some(monitor) = select_monitor(&config.monitor_selector) {
        window.set_monitor(Some(&monitor));
        eprintln!("covermint: using monitor {}", monitor_label(&monitor));
    } else {
        eprintln!(
            "covermint: monitor selector '{}' not found, compositor will choose",
            config.monitor_selector
        );
    }

    let picture = gtk::Picture::new();
    picture.set_width_request(config.size);
    picture.set_height_request(config.size);
    picture.set_can_shrink(false);
    picture.set_content_fit(gtk::ContentFit::Contain);

    let frame = gtk::Frame::new(None);
    frame.set_child(Some(&picture));

    window.set_child(Some(&frame));
    window.present();
    window.set_visible(false);

    let current_url = Rc::new(RefCell::new(None::<String>));
    let picture_ref = picture.clone();
    let window_ref = window.clone();
    let config_ref = config.clone();

    let refresh = move || {
        match query_player(&config_ref.player) {
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
                            picture_ref.set_paintable(Some(&texture));
                            *current_url.borrow_mut() = Some(art_url);
                        }
                        None => {
                            eprintln!("covermint: failed to download artwork");
                            picture_ref.set_paintable(Option::<&gdk::Texture>::None);
                            *current_url.borrow_mut() = None;
                            window_ref.set_visible(false);
                            return;
                        }
                    }
                }

                window_ref.set_visible(true);
            }
            _ => {
                picture_ref.set_paintable(Option::<&gdk::Texture>::None);
                *current_url.borrow_mut() = None;
                window_ref.set_visible(false);
            }
        }
    };

    refresh();

    glib::timeout_add_seconds_local(config.poll_seconds, move || {
        refresh();
        glib::ControlFlow::Continue
    });
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
            monitor.description().map(|v| v.to_string()),
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
    let description = monitor.description().map(|value| value.to_string());
    let manufacturer = monitor.manufacturer().map(|value| value.to_string());
    let model = monitor.model().map(|value| value.to_string());

    [connector, description, manufacturer.zip(model).map(|(a, b)| format!("{a} {b}"))]
        .into_iter()
        .flatten()
        .next()
        .unwrap_or_else(|| "unknown monitor".to_string())
}

fn query_player(player: &str) -> Option<MediaState> {
    let status = run_command("playerctl", &["-p", player, "status"])?;
    let status = if status.trim() == "Playing" {
        PlaybackStatus::Playing
    } else {
        PlaybackStatus::NotPlaying
    };

    let art_url = run_command("playerctl", &["-p", player, "metadata", "mpris:artUrl"])
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
