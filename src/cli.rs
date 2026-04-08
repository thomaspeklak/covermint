use std::env;

use crate::model::{Config, Placement, ShellLayer, Transition};

pub(crate) const USAGE: &str = "usage: covermint [--monitor auto|internal|external|0|#0|eDP-1] [--player auto|<name>] [--size 420] [--width 520] [--height 420] [--placement bottom-right] [--offset-x 48] [--offset-y 48] [--margin 48] [--border-width 2] [--border-color 'rgba(255,255,255,0.35)'] [--corner-radius 18] [--opacity 0.92] [--transition fade|flip|hinge|slide|cover|none] [--transition-ms 180] [--poll-seconds 2] [--show-paused] [--no-cache] [--cache-max-files 128] [--cache-max-mb 256] [--layer background|bottom] [--init-config] [--list-monitors] [--list-players] [--help]";

#[derive(Clone, Debug)]
pub(crate) enum StartupAction {
    Help,
    InitConfig,
    ListMonitors,
    ListPlayers,
    Run(Box<Config>),
}

impl StartupAction {
    pub(crate) fn from_env() -> Result<Self, String> {
        let mut config = Config::default();
        if let Err(error) = crate::config::load_external_config(&mut config) {
            eprintln!("covermint: failed to apply config.toml settings: {error}");
        }

        let mut init_config = false;
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
                "--init-config" => init_config = true,
                "--list-monitors" => list_monitors = true,
                "--list-players" => list_players = true,
                "--help" | "-h" => return Ok(Self::Help),
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        if init_config {
            return Ok(Self::InitConfig);
        }

        if list_monitors {
            return Ok(Self::ListMonitors);
        }

        if list_players {
            return Ok(Self::ListPlayers);
        }

        config.validate()?;
        Ok(Self::Run(Box::new(config)))
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
