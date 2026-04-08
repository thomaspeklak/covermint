use gtk::{gdk, prelude::*};

pub(crate) fn list_monitors() {
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

pub(crate) fn select_monitor(selector: &str) -> Option<gdk::Monitor> {
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

pub(crate) fn monitor_label(monitor: &gdk::Monitor) -> String {
    let (connector, _, _) = monitor_parts(monitor);
    match (connector, monitor_description(monitor)) {
        (Some(connector), Some(description)) => format!("{connector} — {description}"),
        (Some(connector), None) => connector,
        (None, Some(description)) => description,
        (None, None) => "unknown monitor".to_string(),
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
