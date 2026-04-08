use gtk::pango::EllipsizeMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetadataSection {
    Top,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SectionAlign {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TruncateMode {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAnimationMode {
    None,
    Typewriter,
    Fade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RevealDirection {
    TopLeftToBottomRight,
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
    BottomRightToTopLeft,
}

#[derive(Clone, Debug)]
pub(crate) struct TextAnimationConfig {
    pub(crate) mode: TextAnimationMode,
    pub(crate) direction: RevealDirection,
    pub(crate) duration_ms: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct MetadataStyleConfig {
    pub(crate) font_family: String,
    pub(crate) font_size_px: i32,
    pub(crate) font_weight: i32,
    pub(crate) text_color: String,
    pub(crate) background_color: String,
    pub(crate) padding_px: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct MetadataSectionConfig {
    pub(crate) enabled: bool,
    pub(crate) template: String,
    pub(crate) align: SectionAlign,
    pub(crate) truncate: TruncateMode,
    pub(crate) band_size_px: i32,
    pub(crate) style: MetadataStyleConfig,
    pub(crate) animation: TextAnimationConfig,
}

#[derive(Clone, Debug)]
pub(crate) struct MetadataConfig {
    pub(crate) enabled: bool,
    pub(crate) top: MetadataSectionConfig,
    pub(crate) left: MetadataSectionConfig,
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
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            other => Err(format!(
                "unsupported align value '{other}', expected start or end"
            )),
        }
    }
}

impl TruncateMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "start" => Ok(Self::Start),
            "end" => Ok(Self::End),
            other => Err(format!(
                "unsupported truncate value '{other}', expected start or end"
            )),
        }
    }

    pub(crate) fn as_ellipsize(self) -> EllipsizeMode {
        match self {
            Self::Start => EllipsizeMode::Start,
            Self::End => EllipsizeMode::End,
        }
    }
}

impl TextAnimationMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
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
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
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
