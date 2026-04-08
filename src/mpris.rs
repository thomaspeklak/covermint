use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, OnceLock, RwLock, mpsc},
    thread,
    time::Duration,
};
use zbus::{
    MatchRule,
    blocking::{Connection, MessageIterator, Proxy, fdo::DBusProxy},
    message::Type as MessageType,
    zvariant::OwnedValue,
};

use crate::model::{MediaState, PlaybackStatus, TrackMetadata};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
const MPRIS_PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
const LISTENER_RETRY_DELAY: Duration = Duration::from_secs(2);
const MIN_RECONCILE_SECONDS: u32 = 30;

#[derive(Clone)]
struct MprisRuntime {
    cache: Arc<RwLock<BTreeMap<String, MediaState>>>,
}

static RUNTIME: OnceLock<MprisRuntime> = OnceLock::new();

pub(crate) fn start_signal_bridge(event_tx: mpsc::Sender<()>, fallback_seconds: u32) {
    RUNTIME.get_or_init(|| {
        let cache = Arc::new(RwLock::new(BTreeMap::new()));
        spawn_controller(
            cache.clone(),
            event_tx,
            fallback_seconds.max(MIN_RECONCILE_SECONDS),
        );
        MprisRuntime { cache }
    });
}

pub(crate) fn snapshot() -> BTreeMap<String, MediaState> {
    cached_snapshot().unwrap_or_else(snapshot_now)
}

pub(crate) fn player_names() -> Vec<String> {
    snapshot().into_keys().collect()
}

pub(crate) fn position_microseconds_now(player_name: &str) -> Option<u64> {
    let connection = Connection::session().ok()?;
    let bus_name = if player_name.starts_with(MPRIS_PREFIX) {
        player_name.to_string()
    } else {
        format!("{MPRIS_PREFIX}{player_name}")
    };

    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        MPRIS_PATH,
        MPRIS_PLAYER_IFACE,
    )
    .ok()?;

    metadata_position_microseconds(&proxy)
}

fn cached_snapshot() -> Option<BTreeMap<String, MediaState>> {
    let runtime = RUNTIME.get()?;
    runtime.cache.read().ok().map(|cache| cache.clone())
}

fn spawn_controller(
    cache: Arc<RwLock<BTreeMap<String, MediaState>>>,
    event_tx: mpsc::Sender<()>,
    fallback_seconds: u32,
) {
    let spawn_result = thread::Builder::new()
        .name("covermint-mpris-controller".to_string())
        .spawn(move || {
            let (trigger_tx, trigger_rx) = mpsc::channel::<()>();
            spawn_name_owner_listener(trigger_tx.clone());
            spawn_properties_listener(trigger_tx);

            let mut current = snapshot_now();
            if let Ok(mut guard) = cache.write() {
                *guard = current.clone();
            }
            let _ = event_tx.send(());

            let fallback_interval = Duration::from_secs(fallback_seconds as u64);
            loop {
                match trigger_rx.recv_timeout(fallback_interval) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {
                        let next = snapshot_now();
                        if next != current {
                            current = next.clone();
                            if let Ok(mut guard) = cache.write() {
                                *guard = next;
                            }
                            let _ = event_tx.send(());
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        });

    if let Err(error) = spawn_result {
        eprintln!("covermint: failed to start MPRIS controller thread: {error}");
    }
}

fn spawn_name_owner_listener(trigger: mpsc::Sender<()>) {
    spawn_listener_thread(
        "covermint-mpris-name-owner",
        "MPRIS name-owner listener",
        trigger,
        name_owner_listener_pass,
    );
}

fn spawn_properties_listener(trigger: mpsc::Sender<()>) {
    spawn_listener_thread(
        "covermint-mpris-properties",
        "MPRIS properties listener",
        trigger,
        properties_listener_pass,
    );
}

fn spawn_listener_thread(
    thread_name: &str,
    listener_label: &str,
    trigger: mpsc::Sender<()>,
    run_once: fn(&mpsc::Sender<()>) -> Result<(), String>,
) {
    let thread_name = thread_name.to_string();
    let listener_label = listener_label.to_string();
    let listener_label_for_thread = listener_label.clone();

    let spawn_result = thread::Builder::new().name(thread_name).spawn(move || {
        loop {
            if let Err(error) = run_once(&trigger) {
                eprintln!("covermint: {listener_label_for_thread} restarting: {error}");
                thread::sleep(LISTENER_RETRY_DELAY);
            }
        }
    });

    if let Err(error) = spawn_result {
        eprintln!("covermint: failed to start {listener_label}: {error}");
    }
}

fn name_owner_listener_pass(trigger: &mpsc::Sender<()>) -> Result<(), String> {
    let connection = Connection::session().map_err(|error| error.to_string())?;

    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender("org.freedesktop.DBus")
        .map_err(|error| error.to_string())?
        .interface("org.freedesktop.DBus")
        .map_err(|error| error.to_string())?
        .member("NameOwnerChanged")
        .map_err(|error| error.to_string())?
        .build();

    let mut iterator = MessageIterator::for_match_rule(rule, &connection, Some(128))
        .map_err(|error| format!("failed to subscribe to NameOwnerChanged stream: {error}"))?;

    for message in &mut iterator {
        let message = message.map_err(|error| error.to_string())?;

        if let Ok((name, _, _)) = message.body().deserialize::<(String, String, String)>()
            && name.starts_with(MPRIS_PREFIX)
        {
            let _ = trigger.send(());
        }
    }

    Err("NameOwnerChanged stream ended".to_string())
}

fn properties_listener_pass(trigger: &mpsc::Sender<()>) -> Result<(), String> {
    let connection = Connection::session().map_err(|error| error.to_string())?;

    let rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .interface("org.freedesktop.DBus.Properties")
        .map_err(|error| error.to_string())?
        .member("PropertiesChanged")
        .map_err(|error| error.to_string())?
        .path(MPRIS_PATH)
        .map_err(|error| error.to_string())?
        .build();

    let mut iterator = MessageIterator::for_match_rule(rule, &connection, Some(256))
        .map_err(|error| format!("failed to subscribe to PropertiesChanged stream: {error}"))?;

    for message in &mut iterator {
        let message = message.map_err(|error| error.to_string())?;
        let is_player_update = message
            .body()
            .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
            .map(|(interface_name, _, _)| interface_name == MPRIS_PLAYER_IFACE)
            .unwrap_or(true);

        if is_player_update {
            let _ = trigger.send(());
        }
    }

    Err("PropertiesChanged stream ended".to_string())
}

pub(crate) fn snapshot_now() -> BTreeMap<String, MediaState> {
    let connection = match Connection::session() {
        Ok(connection) => connection,
        Err(error) => {
            eprintln!("covermint: failed to connect to session D-Bus for MPRIS: {error}");
            return BTreeMap::new();
        }
    };

    let player_names = list_mpris_bus_names(&connection);
    let mut snapshot = BTreeMap::new();

    for bus_name in player_names {
        let player_name = normalize_player_name(&bus_name);
        if let Some(state) = query_player_state(&connection, &bus_name) {
            snapshot.insert(player_name, state);
        }
    }

    snapshot
}

fn list_mpris_bus_names(connection: &Connection) -> Vec<String> {
    let proxy = match DBusProxy::new(connection) {
        Ok(proxy) => proxy,
        Err(error) => {
            eprintln!("covermint: failed to create DBus proxy: {error}");
            return Vec::new();
        }
    };

    proxy
        .list_names()
        .map(|names| {
            names
                .into_iter()
                .map(|name| name.as_str().to_string())
                .filter(|name| name.starts_with(MPRIS_PREFIX))
                .collect()
        })
        .unwrap_or_else(|error| {
            eprintln!("covermint: failed to list MPRIS names: {error}");
            Vec::new()
        })
}

fn normalize_player_name(bus_name: &str) -> String {
    bus_name
        .strip_prefix(MPRIS_PREFIX)
        .unwrap_or(bus_name)
        .to_string()
}

fn query_player_state(connection: &Connection, bus_name: &str) -> Option<MediaState> {
    let proxy = Proxy::new(connection, bus_name, MPRIS_PATH, MPRIS_PLAYER_IFACE).ok()?;

    let status_raw: String = proxy.get_property("PlaybackStatus").ok()?;
    let status = parse_playback_status(&status_raw);

    let metadata_map: HashMap<String, OwnedValue> =
        proxy.get_property("Metadata").unwrap_or_default();

    let length_microseconds = metadata_length_microseconds(&metadata_map);
    let position_microseconds = metadata_position_microseconds(&proxy);

    Some(MediaState {
        status,
        art_url: metadata_map
            .get("mpris:artUrl")
            .and_then(value_to_string)
            .filter(|value| !value.is_empty()),
        metadata: TrackMetadata {
            artist: metadata_artist(&metadata_map),
            title: metadata_map
                .get("xesam:title")
                .and_then(value_to_string)
                .unwrap_or_default(),
            album: metadata_map
                .get("xesam:album")
                .and_then(value_to_string)
                .unwrap_or_default(),
            track_number: metadata_track_number(&metadata_map),
            length: length_microseconds
                .map(format_timestamp_microseconds)
                .unwrap_or_default(),
            length_microseconds,
            position: position_microseconds
                .map(format_timestamp_microseconds)
                .unwrap_or_default(),
            position_microseconds,
        },
    })
}

fn parse_playback_status(value: &str) -> PlaybackStatus {
    match value.trim() {
        "Playing" => PlaybackStatus::Playing,
        "Paused" => PlaybackStatus::Paused,
        _ => PlaybackStatus::NotPlaying,
    }
}

fn metadata_artist(metadata: &HashMap<String, OwnedValue>) -> String {
    let Some(value) = metadata.get("xesam:artist") else {
        return String::new();
    };

    if let Some(names) = try_owned::<Vec<String>>(value) {
        return names
            .into_iter()
            .find(|entry| !entry.trim().is_empty())
            .unwrap_or_default();
    }

    value_to_string(value).unwrap_or_default()
}

fn metadata_track_number(metadata: &HashMap<String, OwnedValue>) -> String {
    let Some(value) = metadata.get("xesam:trackNumber") else {
        return String::new();
    };

    value_to_string(value)
        .or_else(|| value_to_i64(value).map(|number| number.to_string()))
        .unwrap_or_default()
}

fn metadata_length_microseconds(metadata: &HashMap<String, OwnedValue>) -> Option<u64> {
    let value = metadata.get("mpris:length")?;

    value_to_i64(value)
        .and_then(|raw| u64::try_from(raw).ok())
        .or_else(|| value_to_u64(value))
}

fn metadata_position_microseconds(proxy: &Proxy<'_>) -> Option<u64> {
    proxy
        .get_property::<i64>("Position")
        .ok()
        .and_then(|raw| u64::try_from(raw).ok())
        .or_else(|| proxy.get_property::<u64>("Position").ok())
}

fn try_owned<T>(value: &OwnedValue) -> Option<T>
where
    T: TryFrom<OwnedValue>,
{
    value
        .try_clone()
        .ok()
        .and_then(|owned| T::try_from(owned).ok())
}

fn value_to_string(value: &OwnedValue) -> Option<String> {
    try_owned::<String>(value)
        .or_else(|| <&str>::try_from(value).ok().map(str::to_string))
        .filter(|candidate| !candidate.is_empty())
}

fn value_to_i64(value: &OwnedValue) -> Option<i64> {
    try_owned::<i64>(value)
        .or_else(|| try_owned::<i32>(value).map(i64::from))
        .or_else(|| try_owned::<u64>(value).and_then(|raw| i64::try_from(raw).ok()))
        .or_else(|| try_owned::<u32>(value).map(i64::from))
}

fn value_to_u64(value: &OwnedValue) -> Option<u64> {
    try_owned::<u64>(value)
        .or_else(|| try_owned::<i64>(value).and_then(|raw| u64::try_from(raw).ok()))
        .or_else(|| try_owned::<u32>(value).map(u64::from))
        .or_else(|| try_owned::<i32>(value).and_then(|raw| u64::try_from(raw).ok()))
}

pub(crate) fn format_timestamp_microseconds(microseconds: u64) -> String {
    let total_seconds = microseconds / 1_000_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}
