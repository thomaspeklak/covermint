pub(crate) fn format_timestamp_microseconds(microseconds: u64) -> String {
    let total_seconds = microseconds / 1_000_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

pub(crate) fn parse_timestamp_seconds(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (minutes, seconds) = trimmed.rsplit_once(':')?;
    let minutes = minutes.trim().parse::<u64>().ok()?;
    let seconds = seconds.trim().parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }

    Some(minutes.saturating_mul(60).saturating_add(seconds))
}

pub(crate) fn parse_timestamp_microseconds(value: &str) -> Option<u64> {
    parse_timestamp_seconds(value).map(|seconds| seconds.saturating_mul(1_000_000))
}
