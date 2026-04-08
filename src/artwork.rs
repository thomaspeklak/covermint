use gtk::{gdk, glib};
use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    time::{Duration, SystemTime},
};

use crate::model::Config;

pub(crate) fn load_texture(bytes: Vec<u8>) -> Option<gdk::Texture> {
    let bytes = glib::Bytes::from_owned(bytes);
    gdk::Texture::from_bytes(&bytes).ok()
}

pub(crate) fn download_texture(url: &str, config: &Config) -> Option<gdk::Texture> {
    if config.cache_enabled
        && let Some(path) = cache_path(url)
    {
        if let Ok(bytes) = fs::read(&path) {
            if let Some(texture) = load_texture(bytes.clone()) {
                let _ = fs::write(&path, &bytes);
                return Some(texture);
            }
            let _ = fs::remove_file(&path);
        }

        let bytes = artwork_bytes(url)?;
        let _ = fs::write(&path, &bytes);
        if let Some(dir) = path.parent() {
            trim_cache(
                &dir.to_path_buf(),
                config.cache_max_files,
                config.cache_max_bytes,
            );
        }
        return load_texture(bytes);
    }

    load_texture(artwork_bytes(url)?)
}

fn cache_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("covermint").join("artwork"))
}

fn cache_path(url: &str) -> Option<PathBuf> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }

    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);

    let dir = cache_dir()?;
    fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("{:016x}.img", hasher.finish())))
}

fn trim_cache(dir: &PathBuf, max_files: usize, max_bytes: u64) {
    const MAX_CACHE_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30);

    let now = SystemTime::now();
    let mut entries = Vec::new();
    let mut total_bytes = 0_u64;

    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }

        let modified = metadata.modified().ok();
        let is_stale = modified
            .and_then(|time| now.duration_since(time).ok())
            .map(|age| age > MAX_CACHE_AGE)
            .unwrap_or(false);

        if is_stale {
            let _ = fs::remove_file(entry.path());
            continue;
        }

        let size = metadata.len();
        total_bytes = total_bytes.saturating_add(size);
        entries.push((modified, entry.path(), size));
    }

    entries.sort_by_key(|(modified, _, _)| *modified);

    while entries.len() > max_files || total_bytes > max_bytes {
        let Some((_, path, size)) = entries.first().cloned() else {
            break;
        };
        entries.remove(0);
        total_bytes = total_bytes.saturating_sub(size);
        let _ = fs::remove_file(path);
    }
}

fn artwork_bytes(url: &str) -> Option<Vec<u8>> {
    let parsed = reqwest::Url::parse(url).ok()?;
    match parsed.scheme() {
        "file" => fs::read(parsed.to_file_path().ok()?).ok(),
        "http" | "https" => {
            let response = reqwest::blocking::get(url).ok()?.error_for_status().ok()?;
            Some(response.bytes().ok()?.to_vec())
        }
        _ => None,
    }
}
