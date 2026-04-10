use serde::Deserialize;
use std::{env, fs, path::PathBuf};

use crate::model::{
    ArtworkFit, Config, LyricsConfig, LyricsLayout, MetadataConfig, MetadataSectionConfig,
    MetadataStyleConfig, Placement, RevealDirection, SectionAlign, ShellLayer, TextAnimationConfig,
    TextAnimationMode, Transition, TruncateMode,
};

const DEFAULT_CONFIG_TOML: &str = include_str!("../contrib/config/covermint.config.toml");

#[derive(Deserialize, Default)]
struct FileConfig {
    monitor: Option<String>,
    player: Option<String>,
    size: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
    artwork_fit: Option<String>,
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
    lyrics: Option<FileLyrics>,
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

#[derive(Deserialize, Default)]
struct FileLyrics {
    enabled: Option<bool>,
    layout: Option<String>,
    lines_visible: Option<usize>,
    panel_width: Option<i32>,
    smooth_scroll: Option<bool>,
    font_family: Option<String>,
    font_size_px: Option<i32>,
    text_color: Option<String>,
    active_line_color: Option<String>,
    background_color: Option<String>,
    padding_px: Option<i32>,
}

fn config_file_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("covermint").join("config.toml"))
}

pub(crate) fn init_config_file() -> Result<PathBuf, String> {
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

pub(crate) fn load_external_config(config: &mut Config) -> Result<(), String> {
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
    if let Some(artwork_fit) = file.artwork_fit.as_ref() {
        config.artwork_fit = ArtworkFit::parse(artwork_fit)?;
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

    if let Some(lyrics) = file.lyrics {
        apply_lyrics_override(&mut config.lyrics, &lyrics)?;
        if let Some(enabled) = lyrics.enabled {
            config.lyrics.enabled = enabled;
        }
    }

    if config.metadata.top.enabled
        && let Err(error) =
            crate::metadata::template::validate_template(&config.metadata.top.template)
    {
        eprintln!("covermint: invalid metadata.top template ({error}); using default '{{artist}}'");
        config.metadata.top.template = MetadataConfig::default().top.template;
    }

    if config.metadata.left.enabled
        && let Err(error) =
            crate::metadata::template::validate_template(&config.metadata.left.template)
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

fn apply_lyrics_override(config: &mut LyricsConfig, source: &FileLyrics) -> Result<(), String> {
    if let Some(layout) = source.layout.as_ref() {
        config.layout = LyricsLayout::parse(layout)?;
    }
    if let Some(lines_visible) = source.lines_visible {
        config.lines_visible = lines_visible.max(1);
    }
    if let Some(panel_width) = source.panel_width {
        config.panel_width = panel_width.max(120);
    }
    if let Some(smooth_scroll) = source.smooth_scroll {
        config.smooth_scroll = smooth_scroll;
    }

    if let Some(font_family) = source.font_family.as_ref() {
        config.style.font_family = font_family.clone();
    }
    if let Some(font_size_px) = source.font_size_px {
        config.style.font_size_px = font_size_px.max(1);
    }
    if let Some(text_color) = source.text_color.as_ref() {
        config.style.text_color = text_color.clone();
    }
    if let Some(active_line_color) = source.active_line_color.as_ref() {
        config.style.active_line_color = active_line_color.clone();
    }
    if let Some(background_color) = source.background_color.as_ref() {
        config.style.background_color = background_color.clone();
    }
    if let Some(padding_px) = source.padding_px {
        config.style.padding_px = padding_px.max(0);
    }

    Ok(())
}
