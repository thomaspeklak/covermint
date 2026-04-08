use crate::model::{MetadataConfig, TrackMetadata, TruncateMode};

#[derive(Clone, Debug)]
pub(crate) struct SectionRender {
    pub(crate) text: String,
    pub(crate) truncate: TruncateMode,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderedMetadata {
    pub(crate) top: Option<SectionRender>,
    pub(crate) left: Option<SectionRender>,
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
    Position,
}

impl TemplateField {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "artist" => Some(Self::Artist),
            "title" => Some(Self::Title),
            "album" => Some(Self::Album),
            "trackNumber" => Some(Self::TrackNumber),
            "length" => Some(Self::Length),
            "position" | "timestamp" => Some(Self::Position),
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
            Self::Position => &metadata.position,
        }
    }
}

pub(crate) fn validate_template(template: &str) -> Result<(), String> {
    compile_template(template).map(|_| ())
}

pub(crate) fn render_metadata(
    config: &MetadataConfig,
    metadata: &TrackMetadata,
) -> RenderedMetadata {
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
