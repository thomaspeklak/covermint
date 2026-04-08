use gtk::{graphene, gsk, prelude::*};
use std::{cell::RefCell, rc::Rc};

use crate::model::{
    MetadataConfig, MetadataSection, MetadataSectionConfig, SectionAlign, TruncateMode,
};

use super::{
    animation::{animate_metadata_text, stop_text_animation},
    template::{RenderedMetadata, SectionRender},
};

#[derive(Clone)]
pub(crate) struct AnimatedMetadataLabel {
    pub(crate) wrapper: gtk::Box,
    pub(crate) left_rotation_stage: Option<gtk::Fixed>,
    pub(crate) label: gtk::Label,
    pub(crate) section: MetadataSection,
    pub(crate) extent_hint: i32,
    pub(crate) animation_source: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    pub(crate) current_text: Rc<RefCell<String>>,
}

#[derive(Clone)]
pub(crate) struct MetadataWidgets {
    pub(crate) top: Option<AnimatedMetadataLabel>,
    pub(crate) left: Option<AnimatedMetadataLabel>,
}

pub(crate) fn new_metadata_label(
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
            wrapper.set_overflow(gtk::Overflow::Hidden);

            label.set_halign(gtk::Align::Fill);
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

pub(crate) fn clear_metadata_widgets(widgets: &MetadataWidgets) {
    if let Some(top) = widgets.top.as_ref() {
        reset_metadata_label(top);
    }

    if let Some(left) = widgets.left.as_ref() {
        reset_metadata_label(left);
    }
}

pub(crate) fn update_metadata_widgets(
    widgets: &MetadataWidgets,
    config: &MetadataConfig,
    rendered: RenderedMetadata,
) {
    if let Some(widget) = widgets.top.as_ref() {
        if let Some(section_render) = rendered.top.as_ref() {
            update_single_metadata_label(widget, &config.top, section_render);
        } else {
            reset_metadata_label(widget);
        }
    }

    if let Some(widget) = widgets.left.as_ref() {
        if let Some(section_render) = rendered.left.as_ref() {
            update_single_metadata_label(widget, &config.left, section_render);
        } else {
            reset_metadata_label(widget);
        }
    }
}

fn reset_metadata_label(widget: &AnimatedMetadataLabel) {
    stop_text_animation(widget);
    widget.label.set_markup("");
    *widget.current_text.borrow_mut() = String::new();
}

fn left_rotation_transform(cover_extent: i32) -> gsk::Transform {
    gsk::Transform::new()
        .translate(&graphene::Point::new(0.0, cover_extent.max(1) as f32))
        .rotate(-90.0)
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
