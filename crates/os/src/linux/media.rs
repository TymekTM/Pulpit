//! Now-playing media on Linux through MPRIS over D-Bus, spoken with
//! `busctl --user` (ships with systemd). Mirrors the Windows SMTC
//! module's behaviour: the same action names, the same snapshot shape
//! (built by the shared [`crate::media`] helpers), the same 1 s poll
//! and push gating, art bytes only when their digest changes.
//!
//! Every busctl value is JSON-wrapped as `{"type":"…","data":…}`; the
//! [`unwrap_busctl`] helper peels that once, nested variants included.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::linux::{run_tool, TOOL_TIMEOUT};
use crate::media::{
    art_digest, attach_art_data, aumid_to_friendly_name, build_snapshot_keyed, AutoRepeatMode,
    MediaProperties, PlaybackInfo, PlaybackStatus, PushGate, TimelineProperties,
};
use crate::{OsError, Result};

const MISSING: &str = "media: busctl not found";
const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYER_PATH: &str = "/org/mpris/MediaPlayer2";

/// One `busctl --user --json=short call` with the fixed leading args.
fn busctl(args: &[&str]) -> Result<String> {
    let mut full: Vec<&str> = vec!["--user", "--json=short", "call"];
    full.extend_from_slice(args);
    let out = run_tool(MISSING, "busctl", &full, None, TOOL_TIMEOUT)?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

// ------------------------------------------------------------- unwrapping

/// Peel one `{"type":"…","data":…}` wrapper; non-wrapped values pass
/// through (defensive against busctl format tweaks).
pub(crate) fn unwrap_busctl(value: &Value) -> &Value {
    value.get("data").unwrap_or(value)
}

/// An integer of either sign: busctl prints signed (`x`) and unsigned
/// (`t`) values as bare JSON numbers - `as_i64` covers negatives,
/// `as_u64` the ones past i64's range (those stay out of scope).
pub(crate) fn as_int(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|u| i64::try_from(u).ok()))
}

/// The dict of a `GetAll s <interface>` reply: `{"type":"a{sv}","data":
/// [{…dict…}]}` -> the dict object (owned: busctl replies are small and
/// parsed once per poll).
pub(crate) fn get_all_dict(reply: &str) -> Option<serde_json::Map<String, Value>> {
    let parsed: Value = serde_json::from_str(reply).ok()?;
    let dict = unwrap_busctl(&parsed).as_array()?.first()?;
    dict.as_object().cloned()
}

/// The inner string of a `Get ss …` reply: `{"type":"v","data":[
/// {"type":"s","data":"Spotify"}]}` -> `Spotify`.
pub(crate) fn get_string(reply: &str) -> Option<String> {
    let parsed: Value = serde_json::from_str(reply).ok()?;
    let variant = unwrap_busctl(&parsed).as_array()?.first()?;
    unwrap_busctl(variant)
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ------------------------------------------------------------ player list

/// Bus names of running MPRIS players, from the D-Bus ListNames call.
pub(crate) fn mpris_players() -> Result<Vec<String>> {
    let reply = busctl(&[
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ListNames",
    ])?;
    parse_bus_names(&reply)
}

/// Pure filter over a ListNames reply: names starting with the MPRIS
/// prefix (the bare prefix itself is not a player).
pub(crate) fn parse_bus_names(reply: &str) -> Result<Vec<String>> {
    let parsed: Value = serde_json::from_str(reply)
        .map_err(|e| OsError::Failed(format!("busctl ListNames: {e}")))?;
    let mut names = Vec::new();
    for name in unwrap_busctl(&parsed)
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str)
    {
        if name.starts_with(MPRIS_PREFIX) && name.len() > MPRIS_PREFIX.len() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

/// Friendly name when the player does not answer `Identity`: the bus
/// name suffix after the MPRIS prefix, without Firefox-style
/// `.instanceN_M` tails, through the shared AUMID mapping.
pub(crate) fn fallback_friendly_name(bus_name: &str) -> String {
    let suffix = bus_name.strip_prefix(MPRIS_PREFIX).unwrap_or(bus_name);
    let base = suffix.split(".instance").next().unwrap_or(suffix);
    aumid_to_friendly_name(base)
}

/// The player's `Identity` property (e.g. "Spotify"), with the bus-name
/// fallback. One extra busctl call; callers cache it.
pub(crate) fn friendly_name(bus_name: &str) -> String {
    busctl(&[
        bus_name,
        PLAYER_PATH,
        "org.freedesktop.DBus.Properties",
        "Get",
        "ss",
        "org.mpris.MediaPlayer2",
        "Identity",
    ])
    .ok()
    .and_then(|reply| get_string(&reply))
    .unwrap_or_else(|| fallback_friendly_name(bus_name))
}

// ---------------------------------------------------------- player state

/// Everything one poll knows about one player: the snapshot inputs the
/// shared builder wants, plus the fields `control` needs (track id for
/// seeking, art URL for the cover).
#[derive(Clone)]
pub(crate) struct PlayerSnapshot {
    pub bus_name: String,
    pub friendly: String,
    pub props: MediaProperties,
    pub playback: PlaybackInfo,
    pub timeline: TimelineProperties,
    pub art_url: Option<String>,
    pub track_id: Option<String>,
}

/// Poll one player's `org.mpris.MediaPlayer2.Player` properties.
fn player_snapshot(bus_name: &str, friendly: &str) -> PlayerSnapshot {
    let reply = busctl(&[
        bus_name,
        PLAYER_PATH,
        "org.freedesktop.DBus.Properties",
        "GetAll",
        "s",
        "org.mpris.MediaPlayer2.Player",
    ])
    .unwrap_or_default();
    let dict = get_all_dict(&reply);
    parse_player(dict.as_ref(), bus_name, friendly)
}

/// Pure mapping from a GetAll dict to the snapshot inputs. Missing keys
/// keep the defaults, like the Windows reader keeps partial reads.
pub(crate) fn parse_player(
    dict: Option<&serde_json::Map<String, Value>>,
    bus_name: &str,
    friendly: &str,
) -> PlayerSnapshot {
    let mut props = MediaProperties::default();
    let mut playback = PlaybackInfo::default();
    let mut timeline = TimelineProperties::default();
    let mut art_url = None;
    let mut track_id = None;

    if let Some(dict) = dict {
        // every top-level value is one busctl wrapper
        let field = |key: &str| dict.get(key).map(unwrap_busctl);

        if let Some(status) = field("PlaybackStatus").and_then(Value::as_str) {
            playback.status = match status {
                "Playing" => PlaybackStatus::Playing,
                "Paused" => PlaybackStatus::Paused,
                "Stopped" => PlaybackStatus::Stopped,
                // MPRIS has no Changing/Opened; unknown stays Closed
                _ => PlaybackStatus::Closed,
            };
            playback.is_playing = playback.status == PlaybackStatus::Playing;
        }
        if let Some(shuffle) = field("Shuffle").and_then(Value::as_bool) {
            playback.shuffle = Some(shuffle);
        }
        if let Some(repeat) = field("LoopStatus").and_then(Value::as_str) {
            playback.repeat = Some(match repeat {
                "Track" => AutoRepeatMode::Track,
                "Playlist" => AutoRepeatMode::List,
                _ => AutoRepeatMode::None,
            });
        }
        playback.can_play = field("CanPlay").and_then(Value::as_bool) == Some(true);
        playback.can_pause = field("CanPause").and_then(Value::as_bool) == Some(true);
        playback.can_skip_next = field("CanGoNext").and_then(Value::as_bool) == Some(true);
        playback.can_skip_prev = field("CanGoPrevious").and_then(Value::as_bool) == Some(true);
        playback.can_stop = field("CanStop").and_then(Value::as_bool) == Some(true);
        playback.can_seek = field("CanSeek").and_then(Value::as_bool) == Some(true);

        // Metadata is an a{sv} inside the outer a{sv}: busctl hands its
        // dict over directly inside the variant; the one-element-array
        // shape the first fixture assumed is accepted too
        let metadata = field("Metadata").and_then(|value| match value {
            Value::Object(_) => Some(value),
            Value::Array(entries) => entries.first(),
            _ => None,
        });
        if let Some(metadata) = metadata {
            let meta = |key: &str| metadata.get(key).map(unwrap_busctl);
            // a string array ("as") is flat in real replies (["Queen"]);
            // the wrapped [["Queen"]] shape stays accepted - the names
            // only matter as the first entry either way
            fn first_string(v: &Value) -> Option<&str> {
                let first = unwrap_busctl(v).as_array()?.first()?;
                match first {
                    Value::String(s) => Some(s.as_str()),
                    Value::Array(names) => names.first().and_then(Value::as_str),
                    _ => None,
                }
            }
            if let Some(title) = meta("xesam:title").and_then(Value::as_str) {
                props.title = title.to_string();
            }
            if let Some(artist) = meta("xesam:artist").and_then(first_string) {
                props.artist = artist.to_string();
            }
            if let Some(album) = meta("xesam:album").and_then(Value::as_str) {
                props.album_title = album.to_string();
            }
            if let Some(album_artist) = meta("xesam:albumArtist").and_then(first_string) {
                props.album_artist = album_artist.to_string();
            }
            if let Some(number) = meta("xesam:trackNumber").and_then(Value::as_i64) {
                props.track_number = Some(number as i32);
            }
            // mpris:length is signed ("x") per the MPRIS spec but a
            // player may answer unsigned ("t"); read either as an
            // integer and keep the negative clamp
            if let Some(length_us) = meta("mpris:length").and_then(as_int) {
                timeline.duration_ms = (length_us.max(0) / 1000) as u64;
                timeline.max_seek_ms = timeline.duration_ms;
            }
            if let Some(url) = meta("mpris:artUrl").and_then(Value::as_str) {
                art_url = Some(url.to_string());
            }
            if let Some(id) = meta("mpris:trackid").and_then(Value::as_str) {
                track_id = Some(id.to_string());
            }
        }

        if let Some(position_us) = field("Position").and_then(Value::as_i64) {
            timeline.position_ms = (position_us.max(0) / 1000) as u64;
        }
    }

    PlayerSnapshot {
        bus_name: bus_name.to_string(),
        friendly: friendly.to_string(),
        props,
        playback,
        timeline,
        art_url,
        track_id,
    }
}

// --------------------------------------------------------------- control

/// The MPRIS method a Windows action name maps to; `None` for names
/// Windows would reject too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MprisAction {
    PlayPause,
    Play,
    Pause,
    Next,
    Previous,
    Stop,
    /// slider 0..1 -> absolute position via SetPosition (needs track id)
    Seek,
}

pub(crate) fn action_of(action: &str) -> Option<MprisAction> {
    match action.trim().to_lowercase().as_str() {
        "play-pause" => Some(MprisAction::PlayPause),
        "play" => Some(MprisAction::Play),
        "pause" => Some(MprisAction::Pause),
        "next" => Some(MprisAction::Next),
        "previous" | "prev" => Some(MprisAction::Previous),
        "stop" => Some(MprisAction::Stop),
        "seek" => Some(MprisAction::Seek),
        _ => None,
    }
}

/// Pure selection over polled states: an explicit target wins (friendly
/// or bus name, case-insensitive substring, like the Windows matcher);
/// otherwise the first Playing player, else the first.
pub(crate) fn select_player<'a>(
    states: &'a [PlayerSnapshot],
    target_app: Option<&str>,
) -> Option<&'a PlayerSnapshot> {
    if let Some(target) = target_app.map(str::trim).filter(|s| !s.is_empty()) {
        let pattern = target.to_lowercase();
        return states.iter().find(|p| {
            p.friendly.to_lowercase().contains(&pattern)
                || p.bus_name.to_lowercase().contains(&pattern)
        });
    }
    states
        .iter()
        .find(|p| p.playback.status == PlaybackStatus::Playing)
        .or_else(|| states.first())
}

pub(crate) fn control(
    target_app: Option<&str>,
    action: &str,
    slider_value: Option<f64>,
) -> Result<bool> {
    let names = mpris_players()?;
    let states = poll_states(&names);
    let player = select_player(&states, target_app)
        .ok_or_else(|| OsError::Failed("no active media session found".to_string()))?;

    let act = action_of(action)
        .ok_or_else(|| OsError::Failed(format!("unsupported media action: {}", action.trim())))?;

    // capability gates mirror the Windows Try*Async booleans: a method
    // the player advertises as disabled is "not handled", not an error
    let capable = match act {
        MprisAction::PlayPause => player.playback.can_play || player.playback.can_pause,
        MprisAction::Play => player.playback.can_play,
        MprisAction::Pause => player.playback.can_pause,
        MprisAction::Next => player.playback.can_skip_next,
        MprisAction::Previous => player.playback.can_skip_prev,
        MprisAction::Stop => player.playback.can_stop,
        MprisAction::Seek => player.playback.can_seek,
    };
    if !capable {
        return Ok(false);
    }

    let mut args: Vec<String> = vec![
        player.bus_name.clone(),
        PLAYER_PATH.to_string(),
        "org.mpris.MediaPlayer2.Player".to_string(),
    ];
    match act {
        MprisAction::Seek => {
            // SetPosition needs the track id the position is relative to
            let Some(track_id) = player.track_id.clone().filter(|s| !s.is_empty()) else {
                return Ok(false);
            };
            let fraction = slider_value.unwrap_or(0.0).clamp(0.0, 1.0);
            let position_us = player.timeline.duration_ms as f64 * 1000.0 * fraction;
            args.push("SetPosition".to_string());
            args.push("ox".to_string());
            args.push(track_id);
            args.push(format!("{}", position_us as i64));
        }
        simple => {
            args.push(
                match simple {
                    MprisAction::PlayPause => "PlayPause",
                    MprisAction::Play => "Play",
                    MprisAction::Pause => "Pause",
                    MprisAction::Next => "Next",
                    MprisAction::Previous => "Previous",
                    MprisAction::Stop => "Stop",
                    MprisAction::Seek => unreachable!("handled above"),
                }
                .to_string(),
            );
        }
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    busctl(&args).map(|_| true)
}

pub(crate) fn list_sessions() -> Result<Vec<String>> {
    let names = mpris_players()?;
    let mut sessions = Vec::new();
    for name in &names {
        let friendly = friendly_name(name);
        if !friendly.is_empty() && !sessions.contains(&friendly) {
            sessions.push(friendly);
        }
    }
    Ok(sessions)
}

fn poll_states(names: &[String]) -> Vec<PlayerSnapshot> {
    let mut states = Vec::with_capacity(names.len());
    for name in names {
        let friendly = friendly_name(name);
        states.push(player_snapshot(name, &friendly));
    }
    states
}

// -------------------------------------------------------------- push loop

/// Poll every second while someone listens (5 s quiet idle otherwise;
/// 10 s when no player exists - the Windows cadence), building the
/// shared snapshot shape and gating sends through the shared PushGate.
/// The thread ends when the receiver is dropped.
pub(crate) fn spawn_push(
    consumers: Arc<dyn Fn() -> usize + Send + Sync + 'static>,
) -> tokio::sync::mpsc::UnboundedReceiver<Value> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("mpris-push".to_string())
        .spawn(move || {
            // Identity lookups only when a player appears, not per poll
            let mut friendly_names: HashMap<String, String> = HashMap::new();
            let mut gate = PushGate::default();
            // digest of the cover the receiver last got bytes for
            let mut last_sent_art_digest: Option<String> = None;

            loop {
                if consumers() == 0 {
                    // quiet idle: nobody renders state
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }

                let names = mpris_players().unwrap_or_default();
                let mut states = Vec::with_capacity(names.len());
                for name in &names {
                    let friendly = friendly_names
                        .entry(name.clone())
                        .or_insert_with(|| friendly_name(name))
                        .clone();
                    states.push(player_snapshot(name, &friendly));
                }
                // the "current session": first playing player, else first
                let current = select_player(&states, None);

                let to_send = match current {
                    Some(player) => {
                        let art = load_art(player.art_url.as_deref());
                        let art_key = art.as_ref().map(|(bytes, _)| art_digest(bytes));
                        let mut snapshot = build_snapshot_keyed(
                            Some(&player.props),
                            Some(&player.playback),
                            Some(&player.timeline),
                            &player.friendly,
                            art_key.as_deref(),
                        );
                        if gate.decide(&snapshot, Instant::now()).send {
                            // bytes go out only with a new cover; an
                            // unchanged digest means the receiver
                            // already holds them
                            if art_key != last_sent_art_digest {
                                if let Some((bytes, ext)) = &art {
                                    attach_art_data(&mut snapshot, bytes, ext);
                                }
                                last_sent_art_digest = art_key;
                            }
                            Some(snapshot)
                        } else {
                            None
                        }
                    }
                    None => {
                        let snapshot = build_snapshot_keyed(None, None, None, "", None);
                        (gate.decide(&snapshot, Instant::now()).send).then_some(snapshot)
                    }
                };

                if let Some(snapshot) = to_send {
                    if tx.send(snapshot).is_err() {
                        break; // receiver dropped
                    }
                }
                std::thread::sleep(poll_interval(current.is_some()));
            }
        })
        .expect("spawn mpris worker thread");
    rx
}

/// Any live player re-polls every second, paused or not; no player at
/// all stays quiet at 10 s (the Windows loop's cadence).
fn poll_interval(any_player: bool) -> Duration {
    if any_player {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(10)
    }
}

/// Read a `file://` cover into bytes (same 5 MB cap as the Windows
/// thumbnail read); `http(s)` art needs a fetcher and stays ignored.
/// Only bytes that sniff as an image travel: a player naming a
/// non-image file yields no art at all, never garbage cover bytes.
fn load_art(url: Option<&str>) -> Option<(Vec<u8>, &'static str)> {
    let path = percent_decode(url?.strip_prefix("file://")?);
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() > 5 * 1024 * 1024 {
        return None;
    }
    let ext = art_ext(&bytes)?;
    Some((bytes, ext))
}

/// The image format of the cover by magic bytes; `None` when the bytes
/// are no image this app can render - unknown content must not reach
/// tablets as a guessed-at jpg.
fn art_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && bytes[8..].starts_with(b"WEBP") {
        Some("webp")
    } else {
        None
    }
}

/// Minimal percent-decoding into bytes (UTF-8 survives; a dangling or
/// invalid escape passes through untouched).
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3).and_then(|pair| {
                std::str::from_utf8(pair)
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
            });
            match hex {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                }
                None => {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // real `busctl --user --json=short call org.freedesktop.DBus …
    // ListNames` output captured on this machine (no player was
    // running: an MPRIS name is spliced in for the filter test)
    const LIST_NAMES: &str = r#"{"type":"as","data":[["org.freedesktop.DBus",":1.0",":1.1","org.freedesktop.Notifications","org.mpris.MediaPlayer2.spotify","org.pulseaudio.Server"]]}"#;

    #[test]
    fn busctl_wrappers_peel_off() {
        let simple: Value = serde_json::from_str(r#"{"type":"s","data":"Spotify"}"#).unwrap();
        assert_eq!(unwrap_busctl(&simple).as_str(), Some("Spotify"));

        let nested: Value =
            serde_json::from_str(r#"{"type":"v","data":[{"type":"s","data":"Spotify"}]}"#).unwrap();
        let inner = unwrap_busctl(&nested).as_array().unwrap().first().unwrap();
        assert_eq!(unwrap_busctl(inner).as_str(), Some("Spotify"));

        // arrays keep their shape: "as" data is [ [ …names ] ]
        let array: Value = serde_json::from_str(r#"{"type":"as","data":[["a","b"]]}"#).unwrap();
        let outer = unwrap_busctl(&array).as_array().unwrap();
        assert_eq!(outer.len(), 1);
        assert_eq!(
            outer.first().unwrap().as_array().unwrap(),
            &["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn get_string_reads_the_identity_variant() {
        assert_eq!(
            get_string(r#"{"type":"v","data":[{"type":"s","data":"Spotify"}]}"#).as_deref(),
            Some("Spotify")
        );
        // empty identities mean "not telling" -> None (caller falls back)
        assert_eq!(
            get_string(r#"{"type":"v","data":[{"type":"s","data":""}]}"#),
            None
        );
        assert_eq!(get_string("not json"), None);
    }

    #[test]
    fn list_names_keeps_only_mpris_players() {
        let names = parse_bus_names(LIST_NAMES).unwrap();
        assert_eq!(names, vec!["org.mpris.MediaPlayer2.spotify"]);
        // the bare prefix is the interface, not a player
        let bare =
            parse_bus_names(r#"{"type":"as","data":[["org.mpris.MediaPlayer2."]]}"#).unwrap();
        assert!(bare.is_empty());
        assert!(parse_bus_names("garbage").is_err());
    }

    #[test]
    fn friendly_name_falls_back_to_the_bus_name_suffix() {
        assert_eq!(
            fallback_friendly_name("org.mpris.MediaPlayer2.firefox.instance_123_456"),
            "Firefox"
        );
        assert_eq!(
            fallback_friendly_name("org.mpris.MediaPlayer2.spotify"),
            "Spotify"
        );
        assert_eq!(fallback_friendly_name("org.mpris.MediaPlayer2.vlc"), "VLC");
    }

    // Real `busctl --user --json=short call org.freedesktop.DBus.Properties
    // GetAll s org.mpris.MediaPlayer2.Player` shape, verified against
    // live busctl output: only the top-level reply wraps its arguments
    // in an array - inside a variant, `data` is the value itself
    // (Metadata's is the dict object, an "as" is a flat array). Values
    // hand-written.
    const GET_ALL: &str = r#"{"type":"a{sv}","data":[{
        "PlaybackStatus":{"type":"s","data":"Playing"},
        "LoopStatus":{"type":"s","data":"Track"},
        "Shuffle":{"type":"b","data":true},
        "Metadata":{"type":"a{sv}","data":{
            "mpris:trackid":{"type":"o","data":"/org/mpris/MediaPlayer2/track/1"},
            "mpris:length":{"type":"x","data":354000000},
            "mpris:artUrl":{"type":"s","data":"file:///home/u/Music/cov%20er.jpg"},
            "xesam:title":{"type":"s","data":"Bohemian Rhapsody"},
            "xesam:artist":{"type":"as","data":["Queen"]},
            "xesam:album":{"type":"s","data":"A Night at the Opera"},
            "xesam:albumArtist":{"type":"as","data":["Queen"]},
            "xesam:trackNumber":{"type":"i","data":11}
        }},
        "Position":{"type":"x","data":60000000},
        "CanGoNext":{"type":"b","data":true},
        "CanGoPrevious":{"type":"b","data":true},
        "CanPlay":{"type":"b","data":true},
        "CanPause":{"type":"b","data":true},
        "CanSeek":{"type":"b","data":true}
    }]}"#;

    #[test]
    fn get_all_dict_is_extracted() {
        assert!(get_all_dict(GET_ALL).is_some());
        assert!(get_all_dict("[]").is_none());
        assert!(get_all_dict("not json").is_none());
    }

    #[test]
    fn metadata_maps_onto_the_shared_snapshot_inputs() {
        let dict = get_all_dict(GET_ALL).expect("dict");
        let player = parse_player(Some(&dict), "org.mpris.MediaPlayer2.spotify", "Spotify");

        assert_eq!(player.props.title, "Bohemian Rhapsody");
        assert_eq!(player.props.artist, "Queen");
        assert_eq!(player.props.album_title, "A Night at the Opera");
        assert_eq!(player.props.album_artist, "Queen");
        assert_eq!(player.props.track_number, Some(11));

        assert_eq!(player.playback.status, PlaybackStatus::Playing);
        assert!(player.playback.is_playing);
        assert_eq!(player.playback.shuffle, Some(true));
        assert_eq!(player.playback.repeat, Some(AutoRepeatMode::Track));
        assert!(player.playback.can_seek && player.playback.can_skip_next);

        assert_eq!(player.timeline.position_ms, 60_000); // µs -> ms
        assert_eq!(player.timeline.duration_ms, 354_000);
        assert_eq!(
            player.art_url.as_deref(),
            Some("file:///home/u/Music/cov%20er.jpg")
        );
        assert_eq!(
            player.track_id.as_deref(),
            Some("/org/mpris/MediaPlayer2/track/1")
        );
    }

    #[test]
    fn the_old_wrapped_metadata_shape_still_parses() {
        // the shape the first fixture assumed: Metadata's data a
        // one-element array, artist arrays one level deeper, and an
        // unsigned "t" length - real busctl does not render this today,
        // but the parser stays tolerant of both
        const WRAPPED: &str = r#"{"type":"a{sv}","data":[{
            "PlaybackStatus":{"type":"s","data":"Paused"},
            "Metadata":{"type":"a{sv}","data":[{
                "xesam:title":{"type":"s","data":"Wrapped Shape"},
                "xesam:artist":{"type":"as","data":[["Queen"]]},
                "xesam:albumArtist":{"type":"as","data":[["Queen"]]},
                "mpris:length":{"type":"t","data":61000000}
            }]}
        }]}"#;
        let dict = get_all_dict(WRAPPED).expect("dict");
        let player = parse_player(Some(&dict), "org.mpris.MediaPlayer2.x", "X");
        assert_eq!(player.props.title, "Wrapped Shape");
        assert_eq!(player.props.artist, "Queen");
        assert_eq!(player.props.album_artist, "Queen");
        assert_eq!(player.playback.status, PlaybackStatus::Paused);
        assert_eq!(player.timeline.duration_ms, 61_000);
    }

    #[test]
    fn paused_status_and_missing_keys_stay_defaults() {
        let paused = r#"{"type":"a{sv}","data":[{
            "PlaybackStatus":{"type":"s","data":"Paused"}
        }]}"#;
        let dict = get_all_dict(paused).unwrap();
        let player = parse_player(Some(&dict), "org.mpris.MediaPlayer2.vlc", "VLC");
        assert_eq!(player.playback.status, PlaybackStatus::Paused);
        assert!(!player.playback.is_playing);
        assert_eq!(player.props.title, "");
        assert_eq!(player.timeline.duration_ms, 0);
        assert!(player.art_url.is_none());

        // a broken reply keeps everything default too
        let empty = parse_player(None, "org.mpris.MediaPlayer2.x", "X");
        assert_eq!(empty.playback.status, PlaybackStatus::Closed);
        assert!(!empty.playback.is_playing);
    }

    #[test]
    fn action_names_map_to_mpris_methods() {
        assert_eq!(action_of("play-pause"), Some(MprisAction::PlayPause));
        assert_eq!(action_of("Play"), Some(MprisAction::Play));
        assert_eq!(action_of("pause"), Some(MprisAction::Pause));
        assert_eq!(action_of("next"), Some(MprisAction::Next));
        assert_eq!(action_of("previous"), Some(MprisAction::Previous));
        assert_eq!(action_of("prev"), Some(MprisAction::Previous));
        assert_eq!(action_of("stop"), Some(MprisAction::Stop));
        assert_eq!(action_of("seek"), Some(MprisAction::Seek));
        // Windows rejects unknown names too; volume is not among the
        // actions its control() accepts either
        assert_eq!(action_of("volume"), None);
        assert_eq!(action_of("shuffle"), None);
        assert_eq!(action_of(""), None);
    }

    fn players_for_selection() -> Vec<PlayerSnapshot> {
        let mut states = Vec::new();
        for (bus, friendly, status) in [
            ("org.mpris.MediaPlayer2.vlc", "VLC", PlaybackStatus::Paused),
            (
                "org.mpris.MediaPlayer2.spotify",
                "Spotify",
                PlaybackStatus::Playing,
            ),
            (
                "org.mpris.MediaPlayer2.firefox.instance_1",
                "Firefox",
                PlaybackStatus::Paused,
            ),
        ] {
            let mut player = parse_player(None, bus, friendly);
            player.playback.status = status;
            player.playback.is_playing = status == PlaybackStatus::Playing;
            states.push(player);
        }
        states
    }

    #[test]
    fn selection_prefers_playing_then_first() {
        let states = players_for_selection();
        assert_eq!(select_player(&states, None).unwrap().friendly, "Spotify");

        let only_paused: Vec<PlayerSnapshot> = vec![states[0].clone()];
        assert_eq!(select_player(&only_paused, None).unwrap().friendly, "VLC");
        assert!(select_player(&[], None).is_none());
    }

    #[test]
    fn selection_matches_target_by_friendly_or_bus_name() {
        let states = players_for_selection();
        // explicit target wins even when another player is playing
        assert_eq!(select_player(&states, Some("vlc")).unwrap().friendly, "VLC");
        // case-insensitive substring on the bus name too
        assert_eq!(
            select_player(&states, Some("FIREFOX")).unwrap().friendly,
            "Firefox"
        );
        // an unmatched target is an error, never a silent fallback
        assert!(select_player(&states, Some("tidal")).is_none());
    }

    #[test]
    fn percent_decoding_handles_art_urls() {
        assert_eq!(percent_decode("/home/u/cov%20er.jpg"), "/home/u/cov er.jpg");
        assert_eq!(percent_decode("/plain/path.png"), "/plain/path.png");
        // a dangling or invalid escape never truncates
        assert_eq!(percent_decode("/50%"), "/50%");
        assert_eq!(percent_decode("/%zz"), "/%zz");
    }

    #[test]
    fn only_image_cover_bytes_ship() {
        // magic bytes only: unknown content is no art, never a jpg guess
        assert_eq!(art_ext(b"plain text, not a cover"), None);
        assert_eq!(art_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(art_ext(&[0x89, b'P', b'N', b'G']), Some("png"));
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(b"VP8 ");
        assert_eq!(art_ext(&webp), Some("webp"));

        // through the file path: text content yields no art, a PNG
        // magic yields png bytes (tempfile dirs, never ~/pulpitApp)
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("cover.txt");
        std::fs::write(&text, "plain text, not a cover").unwrap();
        assert!(load_art(Some(&format!("file://{}", text.display()))).is_none());

        let png = dir.path().join("cover.bin");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\nfake png body").unwrap();
        let (bytes, ext) = load_art(Some(&format!("file://{}", png.display()))).expect("png art");
        assert_eq!(ext, "png");
        assert!(bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
