use gtk::gdk;

use crate::model::{Config, MetadataStyleConfig};

pub(super) fn install_styles(config: &Config) {
    let provider = gtk::CssProvider::new();
    let border_width = config.border_width.max(0);
    let corner_radius = config.corner_radius.max(0);
    let inner_radius = (corner_radius - border_width).max(0);

    let top_style = &config.metadata.top.style;
    let left_style = &config.metadata.left.style;

    let top_label_css = metadata_label_css(".covermint-meta-top", top_style);
    let left_label_css = metadata_label_css(".covermint-meta-left", left_style);

    let css = format!(
        ".covermint-window {{ background-color: transparent; box-shadow: none; border-radius: {corner_radius}px; }}\n\
         .covermint-artwork {{ background-color: transparent; box-shadow: none; border-style: solid; border-width: {border_width}px; border-color: {}; border-radius: {corner_radius}px; }}\n\
         .covermint-artwork-stage {{ background-color: transparent; box-shadow: none; border-radius: {inner_radius}px; }}\n\
         .covermint-meta-top {{ background-color: transparent; min-height: {}px; }}\n\
         .covermint-meta-left {{ background-color: transparent; min-width: {}px; }}\n\
         .covermint-meta-corner {{ background-color: {}; }}\n\
         {top_label_css}\n\
         {left_label_css}",
        config.border_color,
        config.metadata.top.band_size_px.max(0),
        config.metadata.left.band_size_px.max(0),
        top_style.background_color,
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

fn metadata_label_css(selector: &str, style: &MetadataStyleConfig) -> String {
    format!(
        "{selector} .covermint-meta-label {{ color: {}; background-color: {}; padding: {}px; font-family: '{}'; font-size: {}px; font-weight: {}; }}",
        style.text_color,
        style.background_color,
        style.padding_px.max(0),
        style.font_family,
        style.font_size_px.max(1),
        style.font_weight.max(100),
    )
}
