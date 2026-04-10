use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::Read,
    path::PathBuf,
};
use url::form_urlencoded::Serializer;

use crate::{
    model::TrackMetadata,
    timestamp::{parse_timestamp_microseconds, parse_timestamp_seconds},
};

const LRCLIB_BASE: &str = "https://lrclib.net";
const LRCLIB_USER_AGENT: &str = "covermint/0.2.0 (+https://github.com/HazAT/spotpaper)";

#[derive(Clone, Debug)]
pub(crate) struct LyricsSignature {
    cache_key: String,
    track_name: String,
    artist_name: String,
    album_name: String,
    duration_seconds: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) enum LyricsLookupResult {
    Found(SyncedLyrics),
    Missing,
    NotLoaded,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct SyncedLyrics {
    pub(crate) lines: Vec<LyricLine>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct LyricLine {
    pub(crate) at_microseconds: u64,
    pub(crate) text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LyricsCacheEntry {
    cache_key: String,
    missing: bool,
    lines: Vec<LyricLine>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiLyricsRecord {
    track_name: String,
    artist_name: String,
    album_name: Option<String>,
    duration: Option<f64>,
    synced_lyrics: Option<String>,
}

pub(crate) fn signature_from_metadata(metadata: &TrackMetadata) -> Option<LyricsSignature> {
    let track_name = metadata.title.trim().to_string();
    let artist_name = metadata.artist.trim().to_string();

    if track_name.is_empty() || artist_name.is_empty() {
        return None;
    }

    let album_name = metadata.album.trim().to_string();
    let duration_seconds = metadata
        .length_microseconds
        .map(|value| value / 1_000_000)
        .or_else(|| metadata_duration_seconds(metadata))
        .or_else(|| parse_timestamp_microseconds(&metadata.length).map(|value| value / 1_000_000));

    Some(LyricsSignature {
        cache_key: [
            canonicalize(&artist_name),
            canonicalize(&track_name),
            canonicalize(&album_name),
            duration_seconds
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ]
        .join("\u{1f}"),
        track_name,
        artist_name,
        album_name,
        duration_seconds,
    })
}

impl LyricsSignature {
    pub(crate) fn cache_key(&self) -> &str {
        &self.cache_key
    }
}

impl SyncedLyrics {
    pub(crate) fn current_line_index(&self, position_microseconds: u64) -> Option<usize> {
        let index = self
            .lines
            .partition_point(|line| line.at_microseconds <= position_microseconds);

        if index == 0 {
            return None;
        }

        Some(index - 1)
    }

    pub(crate) fn line_text(&self, index: usize) -> Option<&str> {
        self.lines
            .get(index)
            .map(|line| line.text.as_str())
            .filter(|text| !text.trim().is_empty())
    }
}

pub(crate) fn lookup_synced_lyrics(
    signature: &LyricsSignature,
    allow_network: bool,
) -> LyricsLookupResult {
    if let Some(entry) = read_cache_entry(signature) {
        return if entry.missing {
            LyricsLookupResult::Missing
        } else {
            LyricsLookupResult::Found(SyncedLyrics { lines: entry.lines })
        };
    }

    if !allow_network {
        return LyricsLookupResult::NotLoaded;
    }

    if let Some(lyrics) = fetch_synced_lyrics(signature) {
        write_cache_entry(
            signature,
            &LyricsCacheEntry {
                cache_key: signature.cache_key.clone(),
                missing: false,
                lines: lyrics.lines.clone(),
            },
        );
        return LyricsLookupResult::Found(lyrics);
    }

    write_cache_entry(
        signature,
        &LyricsCacheEntry {
            cache_key: signature.cache_key.clone(),
            missing: true,
            lines: Vec::new(),
        },
    );

    LyricsLookupResult::Missing
}

fn fetch_synced_lyrics(signature: &LyricsSignature) -> Option<SyncedLyrics> {
    if let Some(duration_seconds) = signature.duration_seconds
        && !signature.album_name.trim().is_empty()
    {
        if let Some(record) = fetch_get_style_record("/api/get-cached", signature, duration_seconds)
            .or_else(|| fetch_get_style_record("/api/get", signature, duration_seconds))
            && let Some(lyrics) = parse_synced_lyrics(record.synced_lyrics.as_deref())
        {
            return Some(lyrics);
        }
    }

    fetch_search_best_match(signature)
        .and_then(|record| parse_synced_lyrics(record.synced_lyrics.as_deref()))
}

fn fetch_get_style_record(
    endpoint: &str,
    signature: &LyricsSignature,
    duration_seconds: u64,
) -> Option<ApiLyricsRecord> {
    let url = build_url(
        endpoint,
        [
            ("track_name", signature.track_name.as_str()),
            ("artist_name", signature.artist_name.as_str()),
            ("album_name", signature.album_name.as_str()),
            ("duration", &duration_seconds.to_string()),
        ],
    );

    request_json::<ApiLyricsRecord>(&url)
}

fn fetch_search_best_match(signature: &LyricsSignature) -> Option<ApiLyricsRecord> {
    let mut params = vec![
        ("track_name", signature.track_name.as_str()),
        ("artist_name", signature.artist_name.as_str()),
    ];

    if !signature.album_name.trim().is_empty() {
        params.push(("album_name", signature.album_name.as_str()));
    }

    let url = build_url("/api/search", params);
    let records = request_json::<Vec<ApiLyricsRecord>>(&url)?;

    records
        .into_iter()
        .filter(|record| {
            record
                .synced_lyrics
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
        })
        .max_by_key(|record| score_search_match(signature, record))
}

fn score_search_match(signature: &LyricsSignature, record: &ApiLyricsRecord) -> i64 {
    let mut score = 0_i64;

    if canonicalize(&record.track_name) == canonicalize(&signature.track_name) {
        score += 120;
    }
    if canonicalize(&record.artist_name) == canonicalize(&signature.artist_name) {
        score += 120;
    }

    let record_album = record.album_name.as_deref().unwrap_or_default();
    if !signature.album_name.trim().is_empty()
        && canonicalize(record_album) == canonicalize(&signature.album_name)
    {
        score += 40;
    }

    if let Some(expected_duration) = signature.duration_seconds
        && let Some(record_duration) = record.duration
    {
        let rounded = record_duration.round();
        if rounded.is_finite() {
            let rounded = rounded.max(0.0) as u64;
            let delta = rounded.abs_diff(expected_duration);
            if delta <= 2 {
                score += 30;
            }
            score -= i64::try_from(delta.min(45)).unwrap_or(45);
        }
    }

    score
}

fn parse_synced_lyrics(raw: Option<&str>) -> Option<SyncedLyrics> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }

    let mut lines = Vec::<LyricLine>::new();
    for source_line in raw.lines() {
        let mut remainder = source_line.trim();
        let mut timestamps = Vec::<u64>::new();

        while remainder.starts_with('[') {
            let Some(end) = remainder.find(']') else {
                break;
            };
            let token = &remainder[1..end];
            if let Some(timestamp) = parse_lrc_timestamp_microseconds(token) {
                timestamps.push(timestamp);
            }
            remainder = remainder[end + 1..].trim_start();
        }

        if timestamps.is_empty() {
            continue;
        }

        let text = remainder.trim().to_string();
        for timestamp in timestamps {
            lines.push(LyricLine {
                at_microseconds: timestamp,
                text: text.clone(),
            });
        }
    }

    if lines.is_empty() {
        return None;
    }

    lines.sort_by_key(|line| line.at_microseconds);
    lines.dedup_by(|a, b| a.at_microseconds == b.at_microseconds && a.text == b.text);

    Some(SyncedLyrics { lines })
}

fn parse_lrc_timestamp_microseconds(value: &str) -> Option<u64> {
    let token = value.trim();
    if token.is_empty() {
        return None;
    }

    let (minutes_part, seconds_and_fraction) = token.split_once(':')?;
    let minutes = minutes_part.trim().parse::<u64>().ok()?;

    let (seconds, fractional_micros) =
        if let Some((seconds_part, fractional_part)) = seconds_and_fraction.split_once('.') {
            let seconds = seconds_part.trim().parse::<u64>().ok()?;
            let digits: String = fractional_part
                .chars()
                .filter(|value| value.is_ascii_digit())
                .take(6)
                .collect();

            if digits.is_empty() {
                (seconds, 0_u64)
            } else {
                let parsed = digits.parse::<u64>().ok()?;
                let scale = 10_u64.saturating_pow(6_u32.saturating_sub(digits.len() as u32));
                (seconds, parsed.saturating_mul(scale))
            }
        } else {
            (seconds_and_fraction.trim().parse::<u64>().ok()?, 0_u64)
        };

    if seconds >= 60 {
        return None;
    }

    Some(
        minutes
            .saturating_mul(60)
            .saturating_add(seconds)
            .saturating_mul(1_000_000)
            .saturating_add(fractional_micros),
    )
}

fn build_url<'a>(endpoint: &str, params: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut serializer = Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    let query = serializer.finish();

    if query.is_empty() {
        format!("{LRCLIB_BASE}{endpoint}")
    } else {
        format!("{LRCLIB_BASE}{endpoint}?{query}")
    }
}

fn request_json<T: DeserializeOwned>(url: &str) -> Option<T> {
    let response = ureq::get(url)
        .set("User-Agent", LRCLIB_USER_AGENT)
        .set("X-User-Agent", LRCLIB_USER_AGENT)
        .call()
        .ok()?;

    let mut reader = response.into_reader();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).ok()?;
    serde_json::from_slice::<T>(&bytes).ok()
}

fn canonicalize(input: &str) -> String {
    let mut normalized = String::new();
    let mut previous_space = false;

    for character in input.chars().flat_map(char::to_lowercase) {
        let is_word = character.is_ascii_alphanumeric();
        if is_word {
            normalized.push(character);
            previous_space = false;
            continue;
        }

        if !previous_space {
            normalized.push(' ');
            previous_space = true;
        }
    }

    normalized.trim().to_string()
}

fn cache_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;

    Some(base.join("covermint").join("lyrics"))
}

fn cache_path(signature: &LyricsSignature) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    signature.cache_key.hash(&mut hasher);

    let dir = cache_dir()?;
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{:016x}.json", hasher.finish())))
}

fn read_cache_entry(signature: &LyricsSignature) -> Option<LyricsCacheEntry> {
    let path = cache_path(signature)?;
    let bytes = fs::read(&path).ok()?;
    let entry = serde_json::from_slice::<LyricsCacheEntry>(&bytes).ok()?;

    if entry.cache_key != signature.cache_key {
        return None;
    }

    Some(entry)
}

fn write_cache_entry(signature: &LyricsSignature, entry: &LyricsCacheEntry) {
    let Some(path) = cache_path(signature) else {
        return;
    };

    if let Ok(payload) = serde_json::to_vec(entry) {
        let _ = fs::write(path, payload);
    }
}

fn metadata_duration_seconds(metadata: &TrackMetadata) -> Option<u64> {
    parse_timestamp_seconds(&metadata.length)
}
