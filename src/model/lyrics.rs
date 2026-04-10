#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LyricsLayout {
    SingleLine,
    MultiLine,
}

#[derive(Clone, Debug)]
pub(crate) struct LyricsStyleConfig {
    pub(crate) font_family: String,
    pub(crate) font_size_px: i32,
    pub(crate) text_color: String,
    pub(crate) active_line_color: String,
    pub(crate) background_color: String,
    pub(crate) padding_px: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct LyricsConfig {
    pub(crate) enabled: bool,
    pub(crate) layout: LyricsLayout,
    pub(crate) lines_visible: usize,
    pub(crate) panel_width: i32,
    pub(crate) smooth_scroll: bool,
    pub(crate) style: LyricsStyleConfig,
}

impl LyricsLayout {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "singleline" | "single-line" | "single" => Ok(Self::SingleLine),
            "multiline" | "multi-line" | "multi" => Ok(Self::MultiLine),
            other => Err(format!(
                "unsupported lyrics layout '{other}', expected singleline or multiline"
            )),
        }
    }
}

impl Default for LyricsStyleConfig {
    fn default() -> Self {
        Self {
            font_family: "Inter, Sans".to_string(),
            font_size_px: 24,
            text_color: "rgba(255,255,255,0.96)".to_string(),
            active_line_color: "rgba(255,255,255,1.0)".to_string(),
            background_color: "rgba(0,0,0,0.42)".to_string(),
            padding_px: 12,
        }
    }
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            layout: LyricsLayout::SingleLine,
            lines_visible: 7,
            panel_width: 320,
            smooth_scroll: true,
            style: LyricsStyleConfig::default(),
        }
    }
}
