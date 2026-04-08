mod animation;
pub(crate) mod template;
mod widgets;

pub(crate) use template::render_metadata;
pub(crate) use widgets::{
    MetadataWidgets, clear_metadata_widgets, new_metadata_label, update_metadata_widgets,
};
