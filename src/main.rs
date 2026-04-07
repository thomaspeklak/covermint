use gtk::{gdk, glib, graphene, gsk, pango::EllipsizeMode, prelude::*};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use serde::Deserialize;
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

const USAGE: &str = "usage: covermint [--monitor auto|internal|external|0|#0|eDP-1] [--player auto|<name>] [--size 420] [--width 520] [--height 420] [--placement bottom-right] [--offset-x 48] [--offset-y 48] [--margin 48] [--border-width 2] [--border-color 'rgba(255,255,255,0.35)'] [--corner-radius 18] [--opacity 0.92] [--transition fade|flip|hinge|slide|cover|none] [--transition-ms 180] [--poll-seconds 2] [--show-paused] [--no-cache] [--cache-max-files 128] [--cache-max-mb 256] [--layer background|bottom] [--init-config] [--list-monitors] [--list-players] [--help]";
const SPLASH_LOGO: &[u8] = include_bytes!("../assets/branding/covermint-logo-grunge.png");
const DEFAULT_CONFIG_TOML: &str = include_str!("../contrib/config/covermint.config.toml");
const STARTUP_SPLASH_MIN_SHOW: Duration = Duration::from_millis(900);
const STARTUP_SPLASH_FADE: Duration = Duration::from_millis(220);
const MEDIA_MISS_GRACE: Duration = Duration::from_secs(5);

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
    metadata: MetadataConfig,
}

#[derive(Clone, Debug)]
enum StartupAction {
    Help,
    InitConfig,
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
    Slide,
    Cover,
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

#[derive(Clone)]
struct ArtworkLayer {
    stage: gtk::Fixed,
    picture: gtk::Picture,
}

#[derive(Clone)]
struct AnimatedMetadataLabel {
    wrapper: gtk::Box,
    left_rotation_stage: Option<gtk::Fixed>,
    label: gtk::Label,
    section: MetadataSection,
    extent_hint: i32,
    animation_source: Rc<RefCell<Option<glib::SourceId>>>,
    current_text: Rc<RefCell<String>>,
}

#[derive(Clone)]
struct MetadataWidgets {
    top: Option<AnimatedMetadataLabel>,
    left: Option<AnimatedMetadataLabel>,
}

#[derive(Debug)]
struct MediaState {
    status: PlaybackStatus,
    art_url: Option<String>,
    metadata: TrackMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TrackMetadata {
    artist: String,
    title: String,
    album: String,
    track_number: String,
    length: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaybackStatus {
    Playing,
    Paused,
    NotPlaying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MetadataSection {
    Top,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SectionAlign {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TruncateMode {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextAnimationMode {
    None,
    Typewriter,
    Fade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RevealDirection {
    TopLeftToBottomRight,
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
    BottomRightToTopLeft,
}

#[derive(Clone, Debug)]
struct TextAnimationConfig {
    mode: TextAnimationMode,
    direction: RevealDirection,
    duration_ms: u32,
}

#[derive(Clone, Debug)]
struct MetadataStyleConfig {
    font_family: String,
    font_size_px: i32,
    font_weight: i32,
    text_color: String,
    background_color: String,
    padding_px: i32,
}

#[derive(Clone, Debug)]
struct MetadataSectionConfig {
    enabled: bool,
    template: String,
    align: SectionAlign,
    truncate: TruncateMode,
    band_size_px: i32,
    style: MetadataStyleConfig,
    animation: TextAnimationConfig,
}

#[derive(Clone, Debug)]
struct MetadataConfig {
    enabled: bool,
    top: MetadataSectionConfig,
    left: MetadataSectionConfig,
}

#[derive(Clone, Debug)]
struct SectionRender {
    text: String,
    truncate: TruncateMode,
}

#[derive(Clone, Debug)]
struct RenderedMetadata {
    top: Option<SectionRender>,
    left: Option<SectionRender>,
}

#[derive(Clone, Debug)]
enum TemplatePiece {
    Text(String),
    Field {
        name: TemplateField,
        truncate: Option<TruncateMode>,
    },
}

#[derive(Clone, Copy, Debug)]
enum TemplateField {
    Artist,
    Title,
    Album,
    TrackNumber,
    Length,
}

#[derive(Deserialize, Default)]
struct FileConfig {
    monitor: Option<String>,
    player: Option<String>,
    size: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
    placement: Option<String>,
    offset_x: Option<i32>,
    offset_y: Option<i32>,
    margin: Option<i32>,
    border_width: Option<i32>,
    border_color: Option<String>,
    corner_radius: Option<i32>,
    opacity: Option<f64>,
    transition: Option<String>,
    transition_ms: Option<u32>,
    poll_seconds: Option<u32>,
    show_paused: Option<bool>,
    no_cache: Option<bool>,
    cache_max_files: Option<usize>,
    cache_max_mb: Option<u64>,
    layer: Option<String>,
    metadata: Option<FileMetadata>,
}

#[derive(Deserialize, Default)]
struct FileMetadata {
    enabled: Option<bool>,
    style: Option<FileMetadataStyle>,
    animation: Option<FileAnimation>,
    top: Option<FileMetadataSection>,
    left: Option<FileMetadataSection>,
}

#[derive(Deserialize, Default)]
struct FileMetadataSection {
    enabled: Option<bool>,
    template: Option<String>,
    align: Option<String>,
    truncate: Option<String>,
    band_size_px: Option<i32>,
    style: Option<FileMetadataStyle>,
    animation: Option<FileAnimation>,
}

#[derive(Deserialize, Default)]
struct FileMetadataStyle {
    font_family: Option<String>,
    font_size_px: Option<i32>,
    font_weight: Option<i32>,
    text_color: Option<String>,
    background_color: Option<String>,
    padding_px: Option<i32>,
}

#[derive(Deserialize, Default)]
struct FileAnimation {
    mode: Option<String>,
    direction: Option<String>,
    duration_ms: Option<u32>,
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

impl Default for MetadataStyleConfig {
    fn default() -> Self {
        Self {
            font_family: "Inter, Sans".to_string(),
            font_size_px: 20,
            font_weight: 700,
            text_color: "rgba(255,255,255,0.94)".to_string(),
            background_color: "rgba(0,0,0,0.30)".to_string(),
            padding_px: 8,
        }
    }
}

impl Default for TextAnimationConfig {
    fn default() -> Self {
        Self {
            mode: TextAnimationMode::Fade,
            direction: RevealDirection::LeftToRight,
            duration_ms: 680,
        }
    }
}

impl Default for MetadataConfig {
    fn default() -> Self {
        let top_style = MetadataStyleConfig::default();
        let left_style = MetadataStyleConfig {
            font_size_px: 18,
            ..MetadataStyleConfig::default()
        };

        Self {
            enabled: true,
            top: MetadataSectionConfig {
                enabled: true,
                template: "{{artist}}".to_string(),
                align: SectionAlign::Start,
                truncate: TruncateMode::End,
                band_size_px: 40,
                style: top_style,
                animation: TextAnimationConfig {
                    mode: TextAnimationMode::Fade,
                    direction: RevealDirection::LeftToRight,
                    duration_ms: 620,
                },
            },
            left: MetadataSectionConfig {
                enabled: true,
                template: "{{title}}".to_string(),
                align: SectionAlign::Start,
                truncate: TruncateMode::End,
                band_size_px: 34,
                style: left_style,
                animation: TextAnimationConfig {
                    mode: TextAnimationMode::Fade,
                    direction: RevealDirection::TopToBottom,
                    duration_ms: 700,
                },
            },
        }
    }
}

impl SectionAlign {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            other => Err(format!(
                "unsupported align value '{other}', expected start or end"
            )),
        }
    }

    fn as_halign(self) -> gtk::Align {
        match self {
            Self::Start => gtk::Align::Start,
            Self::End => gtk::Align::End,
        }
    }
}

impl TruncateMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            other => Err(format!(
                "unsupported truncate value '{other}', expected start or end"
            )),
        }
    }

    fn as_ellipsize(self) -> EllipsizeMode {
        match self {
            Self::Start => EllipsizeMode::Start,
            Self::End => EllipsizeMode::End,
        }
    }
}

impl TextAnimationMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "typewriter" => Ok(Self::Typewriter),
            "fade" => Ok(Self::Fade),
            other => Err(format!(
                "unsupported animation mode '{other}', expected none, typewriter, or fade"
            )),
        }
    }
}

impl RevealDirection {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "tl-br" => Ok(Self::TopLeftToBottomRight),
            "l-r" => Ok(Self::LeftToRight),
            "r-l" => Ok(Self::RightToLeft),
            "t-b" => Ok(Self::TopToBottom),
            "b-t" => Ok(Self::BottomToTop),
            "br-tl" => Ok(Self::BottomRightToTopLeft),
            other => Err(format!(
                "unsupported animation direction '{other}', expected one of: tl-br, l-r, r-l, t-b, b-t, br-tl"
            )),
        }
    }
}

impl TemplateField {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "artist" => Some(Self::Artist),
            "title" => Some(Self::Title),
            "album" => Some(Self::Album),
            "trackNumber" => Some(Self::TrackNumber),
            "length" => Some(Self::Length),
            _ => None,
        }
    }

    fn value(self, metadata: &TrackMetadata) -> &str {
        match self {
            Self::Artist => &metadata.artist,
            Self::Title => &metadata.title,
            Self::Album => &metadata.album,
            Self::TrackNumber => &metadata.track_number,
            Self::Length => &metadata.length,
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

    if matches!(&action, StartupAction::InitConfig) {
        return match init_config_file() {
            Ok(path) => {
                println!("covermint: wrote config template to {}", path.display());
                glib::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("covermint: failed to initialize config: {error}");
                glib::ExitCode::FAILURE
            }
        };
    }

    let app = gtk::Application::builder()
        .application_id("dev.tgz.covermint")
        .build();

    app.connect_activate(move |app| match &action {
        StartupAction::Help | StartupAction::InitConfig => app.quit(),
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
            metadata: MetadataConfig::default(),
        }
    }
}

impl StartupAction {
    fn from_env() -> Result<Self, String> {
        let mut config = Config::default();
        if let Err(error) = load_external_config(&mut config) {
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
        Ok(Self::Run(config))
    }
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        if matches!(self.transition, Transition::Slide | Transition::Cover)
            && !self.placement.supports_edge_anchored_transition()
        {
            return Err(format!(
                "--transition {} requires a placement adjacent to a screen edge, got '{}'",
                self.transition.label(),
                self.placement.label()
            ));
        }

        Ok(())
    }
}

impl Transition {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "fade" => Ok(Self::Fade),
            "flip" => Ok(Self::Flip),
            "hinge" => Ok(Self::Hinge),
            "slide" => Ok(Self::Slide),
            "cover" => Ok(Self::Cover),
            other => Err(format!(
                "unsupported --transition value '{other}', expected one of: none, fade, flip, hinge, slide, cover"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fade => "fade",
            Self::Flip => "flip",
            Self::Hinge => "hinge",
            Self::Slide => "slide",
            Self::Cover => "cover",
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

    fn supports_edge_anchored_transition(self) -> bool {
        self.edge_motion_direction().is_some()
    }

    fn edge_motion_direction(self) -> Option<(f64, f64)> {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => Some((-1.0, 0.0)),
            Self::TopRight | Self::Right | Self::BottomRight => Some((1.0, 0.0)),
            Self::Top => Some((0.0, -1.0)),
            Self::Bottom => Some((0.0, 1.0)),
            Self::Center => None,
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

fn config_file_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("covermint").join("config.toml"))
}

fn init_config_file() -> Result<PathBuf, String> {
    let path = config_file_path().ok_or_else(|| {
        "could not determine config directory (set HOME or XDG_CONFIG_HOME)".to_string()
    })?;

    if path.exists() {
        return Err(format!(
            "{} already exists (delete it first if you want to regenerate)",
            path.display()
        ));
    }

    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid config path: {}", path.display()))?;

    fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    fs::write(&path, DEFAULT_CONFIG_TOML)
        .map_err(|error| format!("{}: {error}", path.display()))?;

    Ok(path)
}

fn load_external_config(config: &mut Config) -> Result<(), String> {
    let Some(path) = config_file_path() else {
        return Ok(());
    };

    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;

    let file = toml::from_str::<FileConfig>(&content)
        .map_err(|error| format!("{}: {error}", path.display()))?;

    if let Some(monitor) = file.monitor.as_ref() {
        config.monitor_selector = monitor.clone();
    }
    if let Some(player) = file.player.as_ref() {
        config.player = player.clone();
    }
    if let Some(size) = file.size {
        config.width = size;
        config.height = size;
    }
    if let Some(width) = file.width {
        config.width = width;
    }
    if let Some(height) = file.height {
        config.height = height;
    }
    if let Some(placement) = file.placement.as_ref() {
        config.placement = Placement::parse(placement)?;
    }
    if let Some(offset_x) = file.offset_x {
        config.offset_x = offset_x;
    }
    if let Some(offset_y) = file.offset_y {
        config.offset_y = offset_y;
    }
    if let Some(margin) = file.margin {
        config.offset_x = margin;
        config.offset_y = margin;
    }
    if let Some(border_width) = file.border_width {
        config.border_width = border_width;
    }
    if let Some(border_color) = file.border_color.as_ref() {
        config.border_color = border_color.clone();
    }
    if let Some(corner_radius) = file.corner_radius {
        config.corner_radius = corner_radius;
    }
    if let Some(opacity) = file.opacity {
        if !(0.0..=1.0).contains(&opacity) {
            return Err(format!(
                "unsupported opacity value '{opacity}', expected a number between 0.0 and 1.0"
            ));
        }
        config.opacity = opacity;
    }
    if let Some(transition) = file.transition.as_ref() {
        config.transition = Transition::parse(transition)?;
    }
    if let Some(transition_ms) = file.transition_ms {
        config.transition_ms = transition_ms;
    }
    if let Some(poll_seconds) = file.poll_seconds {
        config.poll_seconds = poll_seconds;
    }
    if let Some(show_paused) = file.show_paused {
        config.show_paused = show_paused;
    }
    if let Some(no_cache) = file.no_cache {
        config.cache_enabled = !no_cache;
    }
    if let Some(cache_max_files) = file.cache_max_files {
        config.cache_max_files = cache_max_files;
    }
    if let Some(cache_max_mb) = file.cache_max_mb {
        config.cache_max_bytes = cache_max_mb.saturating_mul(1024 * 1024);
    }
    if let Some(layer) = file.layer.as_ref() {
        config.layer = ShellLayer::parse(layer)?;
    }

    if let Some(metadata) = file.metadata {
        if let Some(enabled) = metadata.enabled {
            config.metadata.enabled = enabled;
        }

        if let Some(style) = metadata.style {
            apply_style_override(&mut config.metadata.top.style, &style);
            apply_style_override(&mut config.metadata.left.style, &style);
        }

        if let Some(animation) = metadata.animation {
            apply_animation_override(&mut config.metadata.top.animation, &animation)?;
            apply_animation_override(&mut config.metadata.left.animation, &animation)?;
        }

        if let Some(top) = metadata.top {
            apply_section_override(&mut config.metadata.top, &top)?;
        }

        if let Some(left) = metadata.left {
            apply_section_override(&mut config.metadata.left, &left)?;
        }
    }

    if config.metadata.top.enabled
        && let Err(error) = compile_template(&config.metadata.top.template)
    {
        eprintln!("covermint: invalid metadata.top template ({error}); using default '{{artist}}'");
        config.metadata.top.template = MetadataConfig::default().top.template;
    }

    if config.metadata.left.enabled
        && let Err(error) = compile_template(&config.metadata.left.template)
    {
        eprintln!("covermint: invalid metadata.left template ({error}); using default '{{title}}'");
        config.metadata.left.template = MetadataConfig::default().left.template;
    }

    Ok(())
}

fn apply_section_override(
    section: &mut MetadataSectionConfig,
    source: &FileMetadataSection,
) -> Result<(), String> {
    if let Some(enabled) = source.enabled {
        section.enabled = enabled;
    }
    if let Some(template) = source.template.as_ref() {
        section.template = template.clone();
    }
    if let Some(align) = source.align.as_ref() {
        section.align = SectionAlign::parse(align)?;
    }
    if let Some(truncate) = source.truncate.as_ref() {
        section.truncate = TruncateMode::parse(truncate)?;
    }
    if let Some(size) = source.band_size_px {
        section.band_size_px = size.max(0);
    }
    if let Some(style) = source.style.as_ref() {
        apply_style_override(&mut section.style, style);
    }
    if let Some(animation) = source.animation.as_ref() {
        apply_animation_override(&mut section.animation, animation)?;
    }

    Ok(())
}

fn apply_style_override(style: &mut MetadataStyleConfig, source: &FileMetadataStyle) {
    if let Some(font_family) = source.font_family.as_ref() {
        style.font_family = font_family.clone();
    }
    if let Some(font_size_px) = source.font_size_px {
        style.font_size_px = font_size_px.max(1);
    }
    if let Some(font_weight) = source.font_weight {
        style.font_weight = font_weight.max(100);
    }
    if let Some(text_color) = source.text_color.as_ref() {
        style.text_color = text_color.clone();
    }
    if let Some(background_color) = source.background_color.as_ref() {
        style.background_color = background_color.clone();
    }
    if let Some(padding_px) = source.padding_px {
        style.padding_px = padding_px.max(0);
    }
}

fn apply_animation_override(
    animation: &mut TextAnimationConfig,
    source: &FileAnimation,
) -> Result<(), String> {
    if let Some(mode) = source.mode.as_ref() {
        animation.mode = TextAnimationMode::parse(mode)?;
    }
    if let Some(direction) = source.direction.as_ref() {
        animation.direction = RevealDirection::parse(direction)?;
    }
    if let Some(duration_ms) = source.duration_ms {
        animation.duration_ms = duration_ms;
    }

    Ok(())
}

fn compile_template(template: &str) -> Result<Vec<TemplatePiece>, String> {
    let source = template.replace("\\n", "\n");
    let mut pieces = Vec::new();
    let mut cursor = 0;

    while let Some(open_offset) = source[cursor..].find("{{") {
        let open = cursor + open_offset;
        if open > cursor {
            pieces.push(TemplatePiece::Text(source[cursor..open].to_string()));
        }

        let close = source[open + 2..]
            .find("}}")
            .map(|offset| open + 2 + offset)
            .ok_or_else(|| "unterminated template placeholder".to_string())?;

        let token = source[open + 2..close].trim();
        let (field_name, modifier_name) = token
            .split_once(':')
            .map(|(field, modifier)| (field.trim(), Some(modifier.trim())))
            .unwrap_or((token, None));

        let Some(name) = TemplateField::parse(field_name) else {
            return Err(format!("unknown template field '{{{{{field_name}}}}}'"));
        };

        let truncate = match modifier_name {
            Some("start") => Some(TruncateMode::Start),
            Some("end") => Some(TruncateMode::End),
            Some(other) => {
                return Err(format!(
                    "unsupported template modifier '{other}', expected :start or :end"
                ));
            }
            None => None,
        };

        pieces.push(TemplatePiece::Field { name, truncate });
        cursor = close + 2;
    }

    if cursor < source.len() {
        pieces.push(TemplatePiece::Text(source[cursor..].to_string()));
    }

    if pieces.is_empty() {
        pieces.push(TemplatePiece::Text(String::new()));
    }

    Ok(pieces)
}

fn render_template(
    template: &str,
    metadata: &TrackMetadata,
    fallback_truncate: TruncateMode,
) -> Result<SectionRender, String> {
    let pieces = compile_template(template)?;
    let mut text = String::new();
    let mut truncate = None;

    for piece in pieces {
        match piece {
            TemplatePiece::Text(value) => text.push_str(&value),
            TemplatePiece::Field {
                name,
                truncate: modifier,
            } => {
                text.push_str(name.value(metadata));
                if modifier.is_some() {
                    truncate = modifier;
                }
            }
        }
    }

    Ok(SectionRender {
        text,
        truncate: truncate.unwrap_or(fallback_truncate),
    })
}

fn render_metadata(config: &MetadataConfig, metadata: &TrackMetadata) -> RenderedMetadata {
    if !config.enabled {
        return RenderedMetadata {
            top: None,
            left: None,
        };
    }

    let top = if config.top.enabled {
        render_template(&config.top.template, metadata, config.top.truncate).ok()
    } else {
        None
    };

    let left = if config.left.enabled {
        render_template(&config.left.template, metadata, config.left.truncate).ok()
    } else {
        None
    };

    RenderedMetadata { top, left }
}

fn new_metadata_label(
    section: &MetadataSectionConfig,
    section_kind: MetadataSection,
    cover_extent: i32,
) -> AnimatedMetadataLabel {
    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrapper.add_css_class(match section_kind {
        MetadataSection::Top => "covermint-meta-top",
        MetadataSection::Left => "covermint-meta-left",
    });

    let label = gtk::Label::new(None);
    label.add_css_class("covermint-meta-label");
    label.set_wrap(false);
    label.set_use_markup(true);
    label.set_xalign(match section.align {
        SectionAlign::Start => 0.0,
        SectionAlign::End => 1.0,
    });
    label.set_ellipsize(section.truncate.as_ellipsize());
    label.set_single_line_mode(false);

    let left_rotation_stage = match section_kind {
        MetadataSection::Top => {
            wrapper.set_size_request(cover_extent, section.band_size_px.max(0));
            wrapper.set_halign(gtk::Align::Fill);
            wrapper.set_valign(gtk::Align::Fill);
            wrapper.set_hexpand(true);

            label.set_halign(section.align.as_halign());
            label.set_valign(gtk::Align::Center);
            label.set_hexpand(true);
            wrapper.append(&label);
            None
        }
        MetadataSection::Left => {
            wrapper.set_size_request(section.band_size_px.max(0), -1);
            wrapper.set_halign(gtk::Align::Fill);
            wrapper.set_valign(gtk::Align::Fill);
            wrapper.set_hexpand(false);
            wrapper.set_vexpand(true);
            wrapper.set_overflow(gtk::Overflow::Hidden);

            label.set_halign(gtk::Align::Start);
            label.set_valign(gtk::Align::Start);
            label.set_hexpand(false);
            label.set_size_request(cover_extent.max(1), -1);

            let stage = gtk::Fixed::new();
            stage.set_halign(gtk::Align::Fill);
            stage.set_valign(gtk::Align::Fill);
            stage.set_hexpand(false);
            stage.set_vexpand(true);
            stage.set_overflow(gtk::Overflow::Hidden);
            stage.set_size_request(section.band_size_px.max(0), cover_extent.max(1));
            stage.put(&label, 0.0, 0.0);
            stage.set_child_transform(&label, Some(&left_rotation_transform(cover_extent)));

            wrapper.append(&stage);
            Some(stage)
        }
    };

    AnimatedMetadataLabel {
        wrapper,
        left_rotation_stage,
        label,
        section: section_kind,
        extent_hint: cover_extent,
        animation_source: Rc::new(RefCell::new(None)),
        current_text: Rc::new(RefCell::new(String::new())),
    }
}

fn left_rotation_transform(cover_extent: i32) -> gsk::Transform {
    gsk::Transform::new()
        .translate(&graphene::Point::new(0.0, cover_extent.max(1) as f32))
        .rotate(-90.0)
}

fn clear_metadata_widgets(widgets: &MetadataWidgets) {
    if let Some(top) = widgets.top.as_ref() {
        stop_text_animation(top);
        top.label.set_markup("");
        *top.current_text.borrow_mut() = String::new();
    }

    if let Some(left) = widgets.left.as_ref() {
        stop_text_animation(left);
        left.label.set_markup("");
        *left.current_text.borrow_mut() = String::new();
    }
}

fn update_metadata_widgets(
    widgets: &MetadataWidgets,
    config: &MetadataConfig,
    rendered: RenderedMetadata,
) {
    if let Some(widget) = widgets.top.as_ref() {
        if let Some(section_render) = rendered.top.as_ref() {
            update_single_metadata_label(widget, &config.top, section_render);
        } else {
            stop_text_animation(widget);
            widget.label.set_markup("");
            *widget.current_text.borrow_mut() = String::new();
        }
    }

    if let Some(widget) = widgets.left.as_ref() {
        if let Some(section_render) = rendered.left.as_ref() {
            update_single_metadata_label(widget, &config.left, section_render);
        } else {
            stop_text_animation(widget);
            widget.label.set_markup("");
            *widget.current_text.borrow_mut() = String::new();
        }
    }
}

fn update_single_metadata_label(
    widget: &AnimatedMetadataLabel,
    section: &MetadataSectionConfig,
    rendered: &SectionRender,
) {
    let extent = match widget.section {
        MetadataSection::Top => widget.wrapper.width(),
        MetadataSection::Left => widget.wrapper.height(),
    }
    .max(widget.extent_hint)
    .max(1);

    let truncated = truncate_label_text(&widget.label, &rendered.text, extent, rendered.truncate);
    let display_text = truncated;

    if widget.section == MetadataSection::Left
        && let Some(stage) = widget.left_rotation_stage.as_ref()
    {
        stage.set_size_request(section.band_size_px.max(0), extent);
        widget.label.set_size_request(extent, -1);
        stage.set_child_transform(&widget.label, Some(&left_rotation_transform(extent)));
    }

    if widget.current_text.borrow().as_str() == display_text {
        return;
    }

    stop_text_animation(widget);
    animate_metadata_text(widget, &display_text, section);
}

fn stop_text_animation(widget: &AnimatedMetadataLabel) {
    if let Some(source_id) = widget.animation_source.borrow_mut().take() {
        source_id.remove();
    }
}

fn animate_metadata_text(
    widget: &AnimatedMetadataLabel,
    text: &str,
    section: &MetadataSectionConfig,
) {
    if section.animation.duration_ms == 0 || section.animation.mode == TextAnimationMode::None {
        let markup = markup_for_visible_text(text);
        widget.label.set_markup(&markup);
        *widget.current_text.borrow_mut() = text.to_string();
        return;
    }

    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        widget.label.set_markup("");
        *widget.current_text.borrow_mut() = String::new();
        return;
    }

    let ordered_indices = reveal_order(&chars, section.animation.direction);
    let schedule = match section.animation.mode {
        TextAnimationMode::Typewriter => {
            typewriter_schedule(ordered_indices.len(), section.animation.duration_ms, text)
        }
        _ => even_schedule(ordered_indices.len(), section.animation.duration_ms),
    };

    let label = widget.label.clone();
    let animation_source = widget.animation_source.clone();
    let current_text = widget.current_text.clone();
    let text_owned = text.to_string();
    let final_markup = markup_for_visible_text(&text_owned);
    let target_alpha = target_text_alpha(&section.style.text_color);
    *current_text.borrow_mut() = text_owned.clone();
    let mode = section.animation.mode;
    let start = Instant::now();

    let source_id = glib::timeout_add_local(Duration::from_millis(16), move || {
        let elapsed = start.elapsed().as_millis() as u32;
        let mut visible = vec![false; chars.len()];

        for (order_idx, &char_index) in ordered_indices.iter().enumerate() {
            if elapsed >= schedule[order_idx] {
                visible[char_index] = true;
            }
        }

        let markup = match mode {
            TextAnimationMode::Fade => {
                fade_markup(&chars, &ordered_indices, &schedule, elapsed, target_alpha)
            }
            TextAnimationMode::Typewriter | TextAnimationMode::None => {
                markup_from_visibility(&chars, &visible, target_alpha)
            }
        };

        label.set_markup(&markup);

        if elapsed >= *schedule.last().unwrap_or(&0) + 120 {
            label.set_markup(&final_markup);
            *animation_source.borrow_mut() = None;
            *current_text.borrow_mut() = text_owned.clone();
            return glib::ControlFlow::Break;
        }

        glib::ControlFlow::Continue
    });

    *widget.animation_source.borrow_mut() = Some(source_id);
}

fn reveal_order(chars: &[char], direction: RevealDirection) -> Vec<usize> {
    let mut points = Vec::new();
    let mut x = 0_i32;
    let mut y = 0_i32;

    for (idx, ch) in chars.iter().enumerate() {
        if *ch == '\n' {
            y += 1;
            x = 0;
            continue;
        }

        points.push((idx, x, y));
        x += 1;
    }

    points.sort_by_key(|(_, x, y)| match direction {
        RevealDirection::TopLeftToBottomRight => (x + y, *y, *x),
        RevealDirection::LeftToRight => (*x, *y, 0),
        RevealDirection::RightToLeft => (-*x, *y, 0),
        RevealDirection::TopToBottom => (*y, *x, 0),
        RevealDirection::BottomToTop => (-*y, *x, 0),
        RevealDirection::BottomRightToTopLeft => (-(x + y), -*y, -*x),
    });

    points.into_iter().map(|(idx, _, _)| idx).collect()
}

fn even_schedule(count: usize, duration_ms: u32) -> Vec<u32> {
    if count == 0 {
        return vec![];
    }

    let step = (duration_ms.max(1) as f64 / count as f64).max(1.0);
    (0..count)
        .map(|index| ((index as f64 + 1.0) * step).round() as u32)
        .collect()
}

fn typewriter_schedule(count: usize, duration_ms: u32, text: &str) -> Vec<u32> {
    if count == 0 {
        return vec![];
    }

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let mut seed = hasher.finish();

    let mut weights = Vec::with_capacity(count);
    for _ in 0..count {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let normalized = (seed as f64 / u64::MAX as f64).clamp(0.0, 1.0);
        weights.push(0.35 + normalized * 1.75);
    }

    let total_weight: f64 = weights.iter().sum();
    let mut cumulative = 0.0;
    let mut schedule = Vec::with_capacity(count);

    for weight in weights {
        cumulative += weight;
        schedule.push(((cumulative / total_weight) * duration_ms.max(1) as f64).round() as u32);
    }

    schedule
}

fn fade_markup(
    chars: &[char],
    order: &[usize],
    schedule: &[u32],
    elapsed: u32,
    target_alpha: i32,
) -> String {
    let mut rank = vec![usize::MAX; chars.len()];
    for (position, char_index) in order.iter().enumerate() {
        rank[*char_index] = position;
    }

    let mut markup = String::new();
    for (index, ch) in chars.iter().enumerate() {
        if *ch == '\n' {
            markup.push('\n');
            continue;
        }

        let position = rank[index];
        let start = schedule
            .get(position)
            .copied()
            .unwrap_or_default()
            .saturating_sub(120);
        let end = schedule
            .get(position)
            .copied()
            .unwrap_or_default()
            .max(start + 1);
        let alpha = if elapsed <= start {
            0.0
        } else if elapsed >= end {
            1.0
        } else {
            ((elapsed - start) as f64 / (end - start) as f64).clamp(0.0, 1.0)
        };

        let alpha_value = ((alpha.clamp(0.0, 1.0) * target_alpha as f64).round() as i32).max(1);
        markup.push_str(&format!(
            "<span alpha=\"{alpha_value}\">{}</span>",
            glib::markup_escape_text(&ch.to_string())
        ));
    }

    markup
}

fn markup_from_visibility(chars: &[char], visible: &[bool], target_alpha: i32) -> String {
    let mut markup = String::new();

    for (idx, ch) in chars.iter().enumerate() {
        if *ch == '\n' {
            markup.push('\n');
            continue;
        }

        let alpha_value = if visible.get(idx).copied().unwrap_or(false) {
            target_alpha.max(1)
        } else {
            1
        };

        markup.push_str(&format!(
            "<span alpha=\"{alpha_value}\">{}</span>",
            glib::markup_escape_text(&ch.to_string())
        ));
    }

    markup
}

fn target_text_alpha(text_color: &str) -> i32 {
    gdk::RGBA::parse(text_color)
        .ok()
        .map(|rgba| ((rgba.alpha().clamp(0.0, 1.0) * 65535.0).round() as i32).max(1))
        .unwrap_or(65535)
}

fn markup_for_visible_text(text: &str) -> String {
    text.lines()
        .map(glib::markup_escape_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_label_text(
    label: &gtk::Label,
    text: &str,
    max_extent_px: i32,
    truncate: TruncateMode,
) -> String {
    let max_extent_px = max_extent_px.max(1);

    if text.is_empty() {
        return String::new();
    }

    let fits = |candidate: &str| -> bool {
        let layout = label.create_pango_layout(Some(candidate));
        let (width, _) = layout.pixel_size();
        width <= max_extent_px
    };

    if fits(text) {
        return text.to_string();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut low = 0;
    let mut high = chars.len();
    let mut best = String::new();

    while low <= high {
        let mid = (low + high) / 2;
        let candidate = match truncate {
            TruncateMode::End => {
                let prefix: String = chars.iter().take(mid).collect();
                format!("{prefix}…")
            }
            TruncateMode::Start => {
                let suffix: String = chars.iter().skip(chars.len().saturating_sub(mid)).collect();
                format!("…{suffix}")
            }
        };

        if fits(&candidate) {
            best = candidate;
            low = mid.saturating_add(1);
        } else if mid == 0 {
            break;
        } else {
            high = mid - 1;
        }
    }

    if best.is_empty() {
        "…".to_string()
    } else {
        best
    }
}

fn build_ui(app: &gtk::Application, config: Rc<Config>) {
    let (window_width, window_height) = layout_window_size(&config);
    let (cover_width, cover_height) = cover_frame_size(&config);
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

    let primary_artwork = new_artwork_layer(&config);
    let secondary_artwork = new_artwork_layer(&config);
    let splash_picture = new_splash_picture(&config);
    secondary_artwork.picture.set_opacity(0.0);

    let splash_enabled = if let Some(texture) = load_texture(SPLASH_LOGO.to_vec()) {
        splash_picture.set_paintable(Some(&texture));
        splash_picture.set_visible(true);
        true
    } else {
        eprintln!("covermint: failed to load embedded splash logo");
        false
    };

    let artwork_stack = gtk::Fixed::new();
    artwork_stack.set_size_request(config.width, config.height);
    artwork_stack.set_halign(gtk::Align::Fill);
    artwork_stack.set_valign(gtk::Align::Fill);
    artwork_stack.put(&primary_artwork.stage, 0.0, 0.0);
    artwork_stack.put(&secondary_artwork.stage, 0.0, 0.0);

    let overlay = gtk::Overlay::new();
    overlay.set_size_request(config.width, config.height);
    overlay.set_halign(gtk::Align::Fill);
    overlay.set_valign(gtk::Align::Fill);
    overlay.set_child(Some(&artwork_stack));
    overlay.add_overlay(&splash_picture);

    let artwork_stage = gtk::Box::new(gtk::Orientation::Vertical, 0);
    artwork_stage.add_css_class("covermint-artwork-stage");
    artwork_stage.set_size_request(config.width, config.height);
    artwork_stage.set_halign(gtk::Align::Center);
    artwork_stage.set_valign(gtk::Align::Center);
    artwork_stage.append(&overlay);

    let cover_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    cover_frame.add_css_class("covermint-artwork");
    cover_frame.set_size_request(cover_width, cover_height);
    cover_frame.set_halign(gtk::Align::Fill);
    cover_frame.set_valign(gtk::Align::Fill);
    cover_frame.set_opacity(config.opacity);
    cover_frame.append(&artwork_stage);

    let top_widget = if config.metadata.enabled && config.metadata.top.enabled {
        Some(new_metadata_label(
            &config.metadata.top,
            MetadataSection::Top,
            cover_width,
        ))
    } else {
        None
    };

    let left_widget = if config.metadata.enabled && config.metadata.left.enabled {
        Some(new_metadata_label(
            &config.metadata.left,
            MetadataSection::Left,
            cover_height,
        ))
    } else {
        None
    };

    let left_band = if config.metadata.enabled && config.metadata.left.enabled {
        config.metadata.left.band_size_px.max(0)
    } else {
        0
    };
    let top_band = if config.metadata.enabled && config.metadata.top.enabled {
        config.metadata.top.band_size_px.max(0)
    } else {
        0
    };

    let root = gtk::Fixed::new();
    root.set_size_request(window_width, window_height);
    root.set_halign(gtk::Align::Fill);
    root.set_valign(gtk::Align::Fill);
    root.set_overflow(gtk::Overflow::Hidden);

    root.put(&cover_frame, left_band as f64, top_band as f64);

    if left_band > 0 && top_band > 0 {
        let corner_fill = gtk::Box::new(gtk::Orientation::Vertical, 0);
        corner_fill.add_css_class("covermint-meta-corner");
        corner_fill.set_size_request(left_band, top_band);
        corner_fill.set_halign(gtk::Align::Fill);
        corner_fill.set_valign(gtk::Align::Fill);
        root.put(&corner_fill, 0.0, 0.0);
    }

    if let Some(top) = top_widget.as_ref() {
        root.put(&top.wrapper, left_band as f64, 0.0);
    }

    if let Some(left) = left_widget.as_ref() {
        root.put(&left.wrapper, 0.0, top_band as f64);
    }

    let metadata_widgets = MetadataWidgets {
        top: top_widget,
        left: left_widget,
    };

    window.set_child(Some(&root));
    window.present();
    window.set_visible(splash_enabled);

    let current_url = Rc::new(RefCell::new(None::<String>));
    let active_slot = Rc::new(RefCell::new(ArtworkSlot::Primary));
    let transition_source = Rc::new(RefCell::new(None::<glib::SourceId>));
    let splash_active = Rc::new(RefCell::new(splash_enabled));
    let artwork_stack_ref = artwork_stack.clone();
    let primary_artwork_ref = primary_artwork.clone();
    let secondary_artwork_ref = secondary_artwork.clone();
    let window_ref = window.clone();
    let config_ref = config.clone();
    let monitor_status_ref = monitor_status.clone();
    let splash_active_ref = splash_active.clone();
    let metadata_widgets_ref = metadata_widgets.clone();
    let media_miss_since = Rc::new(RefCell::new(None::<Instant>));

    if splash_enabled {
        schedule_startup_splash_dismissal(&window, &splash_picture, &splash_active, &current_url);
    }

    let refresh = move || {
        sync_window_target(&window_ref, &config_ref, &monitor_status_ref);

        let handle_empty_state = || {
            clear_artwork(
                &primary_artwork_ref,
                &secondary_artwork_ref,
                &active_slot,
                &transition_source,
                &config_ref,
            );
            clear_metadata_widgets(&metadata_widgets_ref);
            *current_url.borrow_mut() = None;

            if !*splash_active_ref.borrow() {
                window_ref.set_visible(false);
            }
        };

        let hold_previous_cover = || {
            if current_url.borrow().is_some() {
                window_ref.set_visible(true);
                reassert_layer_surface(&window_ref, &config_ref);
            } else {
                handle_empty_state();
            }
        };

        let include_metadata = config_ref.metadata.enabled
            && (config_ref.metadata.top.enabled || config_ref.metadata.left.enabled);

        match query_player(&config_ref.player, include_metadata) {
            Some(state) if state.status.should_show_artwork(config_ref.show_paused) => {
                *media_miss_since.borrow_mut() = None;
                let mut has_any_artwork = current_url.borrow().is_some();

                if let Some(art_url) = state.art_url.as_ref() {
                    let needs_reload = current_url
                        .borrow()
                        .as_ref()
                        .map(|current| current != art_url)
                        .unwrap_or(true);

                    if needs_reload {
                        match download_texture(art_url, &config_ref) {
                            Some(texture) => {
                                set_artwork_texture(
                                    &artwork_stack_ref,
                                    &primary_artwork_ref,
                                    &secondary_artwork_ref,
                                    &active_slot,
                                    &transition_source,
                                    &config_ref,
                                    &texture,
                                );
                                *current_url.borrow_mut() = Some(art_url.clone());
                                has_any_artwork = true;
                            }
                            None => {
                                eprintln!(
                                    "covermint: failed to download artwork, keeping previous cover if available"
                                );
                            }
                        }
                    } else {
                        has_any_artwork = true;
                    }
                }

                if !has_any_artwork {
                    handle_empty_state();
                    return;
                }

                let rendered = render_metadata(&config_ref.metadata, &state.metadata);
                update_metadata_widgets(&metadata_widgets_ref, &config_ref.metadata, rendered);

                window_ref.set_visible(true);
                reassert_layer_surface(&window_ref, &config_ref);
            }
            _ => {
                let now = Instant::now();
                let should_clear = {
                    let mut miss_since = media_miss_since.borrow_mut();
                    match *miss_since {
                        Some(started) => now.saturating_duration_since(started) >= MEDIA_MISS_GRACE,
                        None => {
                            *miss_since = Some(now);
                            false
                        }
                    }
                };

                if should_clear {
                    handle_empty_state();
                } else {
                    hold_previous_cover();
                }
            }
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

fn new_artwork_layer(config: &Config) -> ArtworkLayer {
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
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture.set_halign(gtk::Align::Fill);
    picture.set_valign(gtk::Align::Fill);
    picture
}

fn new_splash_picture(config: &Config) -> gtk::Picture {
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

fn run_playerctl<'a>(player: &'a str, args: &[&'a str]) -> Option<String> {
    let mut command_args = Vec::with_capacity(args.len() + 2);
    if !player.eq_ignore_ascii_case("auto") {
        command_args.extend(["-p", player]);
    }
    command_args.extend(args.iter().copied());
    run_command("playerctl", &command_args)
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

#[derive(Clone, Copy)]
struct TransitionFrame {
    width_progress: f64,
    height_progress: f64,
    offset_x: f64,
    offset_y: f64,
    opacity: f64,
}

fn reset_artwork_frame(artwork: &ArtworkLayer, width: i32, height: i32) {
    artwork.picture.set_size_request(width, height);
    artwork.stage.move_(&artwork.picture, 0.0, 0.0);
}

fn cover_frame_size(config: &Config) -> (i32, i32) {
    let border = config.border_width.max(0) * 2;
    (config.width + border, config.height + border)
}

fn layout_window_size(config: &Config) -> (i32, i32) {
    let (cover_width, cover_height) = cover_frame_size(config);

    let left_width = if config.metadata.enabled && config.metadata.left.enabled {
        config.metadata.left.band_size_px.max(0)
    } else {
        0
    };

    let top_height = if config.metadata.enabled && config.metadata.top.enabled {
        config.metadata.top.band_size_px.max(0)
    } else {
        0
    };

    (cover_width + left_width, cover_height + top_height)
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

fn bring_artwork_to_front(stack: &gtk::Fixed, artwork: &ArtworkLayer) {
    stack.remove(&artwork.stage);
    stack.put(&artwork.stage, 0.0, 0.0);
}

fn render_picture_frame(
    artwork: &ArtworkLayer,
    width: i32,
    height: i32,
    transition: Transition,
    frame: TransitionFrame,
) {
    let (frame_width, frame_height, x, y) = match transition {
        Transition::None | Transition::Fade => (width, height, 0.0, 0.0),
        Transition::Flip => {
            let frame_width = scaled_frame_size(width, frame.width_progress, 1.08);
            (frame_width, height, ((width - frame_width) / 2) as f64, 0.0)
        }
        Transition::Hinge => {
            let frame_width = scaled_frame_size(width, frame.width_progress, 1.04);
            let frame_height = scaled_frame_size(height, frame.height_progress, 1.04);
            (
                frame_width,
                frame_height,
                ((width - frame_width) / 2) as f64,
                0.0,
            )
        }
        Transition::Slide | Transition::Cover => (
            width,
            height,
            (width as f64 * frame.offset_x).round(),
            (height as f64 * frame.offset_y).round(),
        ),
    };

    artwork.picture.set_size_request(frame_width, frame_height);
    artwork.stage.move_(&artwork.picture, x, y);
    artwork.picture.set_opacity(frame.opacity.clamp(0.0, 1.0));
}

fn transition_frames(
    transition: Transition,
    placement: Placement,
    progress: f64,
) -> (TransitionFrame, TransitionFrame) {
    let t = progress.clamp(0.0, 1.0);

    match transition {
        Transition::None => (
            TransitionFrame {
                width_progress: 1.0,
                height_progress: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,
                opacity: 0.0,
            },
            TransitionFrame {
                width_progress: 1.0,
                height_progress: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,
                opacity: 1.0,
            },
        ),
        Transition::Fade => {
            let eased = ease_in_out_cubic(t);
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: 0.0,
                    offset_y: 0.0,
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
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 1.0 - (phase * 0.85),
                    },
                    TransitionFrame {
                        width_progress: 0.0,
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                )
            } else {
                let phase = (t - 0.5) / 0.5;
                (
                    TransitionFrame {
                        width_progress: 0.0,
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: ease_out_back_subtle(phase),
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
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
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 1.0 - (phase * 0.9),
                    },
                    TransitionFrame {
                        width_progress: 0.28,
                        height_progress: 0.84,
                        offset_x: 0.0,
                        offset_y: 0.0,
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
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: 0.28 + (spring * 0.72),
                        height_progress: 0.84 + (spring * 0.16),
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: ease_in_out_cubic(phase),
                    },
                )
            }
        }
        Transition::Slide => {
            let eased = ease_in_out_cubic(t);
            let (direction_x, direction_y) = placement
                .edge_motion_direction()
                .expect("validated slide transition placement");
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: direction_x * eased,
                    offset_y: direction_y * eased,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: -direction_x * (1.0 - eased),
                    offset_y: -direction_y * (1.0 - eased),
                    opacity: 1.0,
                },
            )
        }
        Transition::Cover => {
            let eased = ease_in_out_cubic(t);
            let (direction_x, direction_y) = placement
                .edge_motion_direction()
                .expect("validated cover transition placement");
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: direction_x * eased,
                    offset_y: direction_y * eased,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: direction_x * (1.0 - eased),
                    offset_y: direction_y * (1.0 - eased),
                    opacity: 1.0,
                },
            )
        }
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

fn set_artwork_texture(
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

fn clear_artwork(
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

fn schedule_startup_splash_dismissal(
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

fn reassert_layer_surface(window: &gtk::ApplicationWindow, config: &Config) {
    if window.is_visible() && matches!(config.layer, ShellLayer::Background) {
        window.present();
    }
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

    let top_style = &config.metadata.top.style;
    let left_style = &config.metadata.left.style;

    let css = format!(
        ".covermint-window {{ background-color: transparent; box-shadow: none; border-radius: {corner_radius}px; }}\n\
         .covermint-artwork {{ background-color: transparent; box-shadow: none; border-style: solid; border-width: {border_width}px; border-color: {}; border-radius: {corner_radius}px; }}\n\
         .covermint-artwork-stage {{ background-color: transparent; box-shadow: none; border-radius: {inner_radius}px; }}\n\
         .covermint-meta-top {{ background-color: {}; min-height: {}px; padding: {}px; }}\n\
         .covermint-meta-left {{ background-color: {}; min-width: {}px; padding: {}px; }}\n\
         .covermint-meta-corner {{ background-color: {}; }}\n\
         .covermint-meta-top .covermint-meta-label {{ color: {}; font-family: '{}'; font-size: {}px; font-weight: {}; }}\n\
         .covermint-meta-left .covermint-meta-label {{ color: {}; font-family: '{}'; font-size: {}px; font-weight: {}; }}",
        config.border_color,
        top_style.background_color,
        config.metadata.top.band_size_px.max(0),
        top_style.padding_px.max(0),
        left_style.background_color,
        config.metadata.left.band_size_px.max(0),
        left_style.padding_px.max(0),
        top_style.background_color,
        top_style.text_color,
        top_style.font_family,
        top_style.font_size_px.max(1),
        top_style.font_weight.max(100),
        left_style.text_color,
        left_style.font_family,
        left_style.font_size_px.max(1),
        left_style.font_weight.max(100),
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
        if let Some(item) = monitors.item(index)
            && let Ok(monitor) = item.downcast::<gdk::Monitor>()
        {
            all.push(monitor);
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
    if let Ok(index) = selector
        .strip_prefix('#')
        .unwrap_or(selector)
        .parse::<usize>()
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

fn query_named_player(player: &str, include_metadata: bool) -> Option<MediaState> {
    let status = run_playerctl(player, &["status"])?;
    let status = match status.trim() {
        "Playing" => PlaybackStatus::Playing,
        "Paused" => PlaybackStatus::Paused,
        _ => PlaybackStatus::NotPlaying,
    };

    let art_url = run_playerctl(player, &["metadata", "mpris:artUrl"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let metadata = if include_metadata {
        let metadata_blob = run_playerctl(
            player,
            &[
                "metadata",
                "--format",
                "{{xesam:artist}}\n{{xesam:title}}\n{{xesam:album}}\n{{xesam:trackNumber}}\n{{mpris:length}}",
            ],
        )
        .unwrap_or_default();

        let mut fields = metadata_blob.lines();
        let artist = fields.next().map(str::trim).unwrap_or_default().to_string();
        let title = fields.next().map(str::trim).unwrap_or_default().to_string();
        let album = fields.next().map(str::trim).unwrap_or_default().to_string();
        let track_number = fields.next().map(str::trim).unwrap_or_default().to_string();
        let length_raw = fields.next().map(str::trim).unwrap_or_default();

        TrackMetadata {
            artist,
            title,
            album,
            track_number,
            length: format_length_microseconds(length_raw),
        }
    } else {
        TrackMetadata::default()
    };

    Some(MediaState {
        status,
        art_url,
        metadata,
    })
}

fn query_player(player: &str, include_metadata: bool) -> Option<MediaState> {
    if !player.eq_ignore_ascii_case("auto") {
        return query_named_player(player, include_metadata);
    }

    let mut best_match = None;

    for player_name in player_names() {
        let Some(state) = query_named_player(&player_name, include_metadata) else {
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
        .or_else(|| query_named_player(player, include_metadata))
}

fn format_length_microseconds(raw: &str) -> String {
    let Ok(microseconds) = raw.parse::<u64>() else {
        return String::new();
    };

    let total_seconds = microseconds / 1_000_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
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
    if config.cache_enabled
        && let Some(path) = cache_path(url)
    {
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

    load_texture(artwork_bytes(url)?)
}
