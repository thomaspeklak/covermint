use super::MetadataConfig;

#[derive(Clone, Debug)]
pub(crate) struct Config {
    pub(crate) monitor_selector: String,
    pub(crate) player: String,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) artwork_fit: ArtworkFit,
    pub(crate) placement: Placement,
    pub(crate) offset_x: i32,
    pub(crate) offset_y: i32,
    pub(crate) border_width: i32,
    pub(crate) border_color: String,
    pub(crate) corner_radius: i32,
    pub(crate) opacity: f64,
    pub(crate) transition: Transition,
    pub(crate) transition_ms: u32,
    pub(crate) poll_seconds: u32,
    pub(crate) show_paused: bool,
    pub(crate) cache_enabled: bool,
    pub(crate) cache_max_files: usize,
    pub(crate) cache_max_bytes: u64,
    pub(crate) layer: ShellLayer,
    pub(crate) metadata: MetadataConfig,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ShellLayer {
    Background,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ArtworkFit {
    Contain,
    Cover,
    Fill,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Placement {
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
pub(crate) enum AxisPlacement {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transition {
    None,
    Fade,
    Flip,
    Hinge,
    Slide,
    Cover,
}

impl ShellLayer {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "background" => Ok(Self::Background),
            "bottom" => Ok(Self::Bottom),
            other => Err(format!(
                "unsupported --layer value '{other}', expected background or bottom"
            )),
        }
    }
}

impl ArtworkFit {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "contain" => Ok(Self::Contain),
            "cover" => Ok(Self::Cover),
            "fill" => Ok(Self::Fill),
            other => Err(format!(
                "unsupported artwork_fit value '{other}', expected contain, cover, or fill"
            )),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            monitor_selector: "auto".to_string(),
            player: "auto".to_string(),
            width: 420,
            height: 420,
            artwork_fit: ArtworkFit::Cover,
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

impl Config {
    pub(crate) fn validate(&self) -> Result<(), String> {
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
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
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

    pub(crate) fn label(self) -> &'static str {
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
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
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

    pub(crate) fn horizontal(self) -> AxisPlacement {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => AxisPlacement::Start,
            Self::Top | Self::Center | Self::Bottom => AxisPlacement::Center,
            Self::TopRight | Self::Right | Self::BottomRight => AxisPlacement::End,
        }
    }

    pub(crate) fn vertical(self) -> AxisPlacement {
        match self {
            Self::TopLeft | Self::Top | Self::TopRight => AxisPlacement::Start,
            Self::Left | Self::Center | Self::Right => AxisPlacement::Center,
            Self::BottomLeft | Self::Bottom | Self::BottomRight => AxisPlacement::End,
        }
    }

    pub(crate) fn label(self) -> &'static str {
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

    pub(crate) fn supports_edge_anchored_transition(self) -> bool {
        self.edge_motion_direction().is_some()
    }

    pub(crate) fn edge_motion_direction(self) -> Option<(f64, f64)> {
        match self {
            Self::TopLeft | Self::Left | Self::BottomLeft => Some((-1.0, 0.0)),
            Self::TopRight | Self::Right | Self::BottomRight => Some((1.0, 0.0)),
            Self::Top => Some((0.0, -1.0)),
            Self::Bottom => Some((0.0, 1.0)),
            Self::Center => None,
        }
    }
}
