use gtk::{gdk, glib};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};

use crate::model::{MetadataSectionConfig, RevealDirection, TextAnimationMode};

use super::widgets::AnimatedMetadataLabel;

pub(super) fn stop_text_animation(widget: &AnimatedMetadataLabel) {
    if let Some(source_id) = widget.animation_source.borrow_mut().take() {
        source_id.remove();
    }
}

pub(super) fn set_metadata_text_immediate(widget: &AnimatedMetadataLabel, text: &str) {
    stop_text_animation(widget);
    let markup = markup_for_visible_text(text);
    widget.label.set_markup(&markup);
    *widget.current_text.borrow_mut() = text.to_string();
}

pub(super) fn animate_metadata_text(
    widget: &AnimatedMetadataLabel,
    text: &str,
    section: &MetadataSectionConfig,
) {
    if section.animation.duration_ms == 0 || section.animation.mode == TextAnimationMode::None {
        set_metadata_text_immediate(widget, text);
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
