//! System Media Transport Controls (SMTC) integration for Windows.
//!
//! Exposes system-wide "now playing" information and playback control
//! for any active media player (browser YouTube, Tidal, Spotify, VLC,
//! etc.) via Windows Global System Media Transport Controls
//! (Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager).

#[cfg(not(windows))]
use crate::OsError;
use crate::Result;
use serde_json::{json, Value};
use std::sync::Arc;

#[cfg(windows)]
mod win_media;

/// Convert an Application User Model ID (AUMID) or process name to a
/// friendly application title (e.g. "Spotify.exe" -> "Spotify",
/// "F0DC299D809B9700" / "ZenToast-..." -> "Zen Browser").
pub fn aumid_to_friendly_name(aumid: &str) -> String {
    let raw = aumid.trim();
    if raw.is_empty() {
        return String::new();
    }

    // Direct lookup for known common media players / browsers
    let lower = raw.to_lowercase();
    if lower == "spotify.exe" || lower == "spotify" {
        return "Spotify".to_string();
    }
    if lower == "chrome.exe" || lower == "google chrome" || lower.starts_with("chrome.") {
        return "Google Chrome".to_string();
    }
    if lower == "msedge.exe"
        || lower == "microsoft edge"
        || lower.starts_with("microsoft.microsoftedge")
    {
        return "Microsoft Edge".to_string();
    }
    if lower == "firefox.exe" || lower == "mozilla firefox" || lower.starts_with("firefox.") {
        return "Firefox".to_string();
    }
    if lower == "brave.exe" || lower == "brave" {
        return "Brave".to_string();
    }
    if lower == "zen.exe"
        || lower == "zen browser"
        || lower == "f0dc299d809b9700"
        || lower.starts_with("zentoast-")
    {
        return "Zen Browser".to_string();
    }
    if lower == "vlc.exe" || lower == "vlc" {
        return "VLC".to_string();
    }
    if lower == "foobar2000.exe" || lower == "foobar2000" {
        return "foobar2000".to_string();
    }
    if lower == "tidal.exe" || lower == "tidal" {
        return "TIDAL".to_string();
    }
    if lower == "applemusic.exe" || lower.starts_with("appleinc.applemusic") {
        return "Apple Music".to_string();
    }
    if lower == "itunes.exe" || lower == "itunes" {
        return "iTunes".to_string();
    }
    if lower == "musicbee.exe" || lower == "musicbee" {
        return "MusicBee".to_string();
    }
    if lower == "aimp.exe" || lower == "aimp" {
        return "AIMP".to_string();
    }
    if lower.starts_with("microsoft.zunemusic") {
        return "Media Player".to_string();
    }
    if lower.starts_with("microsoft.zunevideo") {
        return "Films & TV".to_string();
    }
    if lower.starts_with("spotifyab.spotifymusic") {
        return "Spotify".to_string();
    }

    // Windows registry check for AppUserModelId DisplayName
    #[cfg(windows)]
    {
        if let Some(name) = win_media::lookup_registry_display_name(raw) {
            return name;
        }
    }

    // Packaged UWP format: Publisher.AppName_hash!AppId
    if let Some((pkg, app)) = raw.split_once('!') {
        let app_trimmed = app.trim();
        if !app_trimmed.is_empty() && !app_trimmed.eq_ignore_ascii_case("app") {
            return humanize_identifier(app_trimmed);
        }
        if let Some((name_part, _)) = pkg.split_once('_') {
            if let Some((_, app_name)) = name_part.split_once('.') {
                return humanize_identifier(app_name);
            }
            return humanize_identifier(name_part);
        }
    }

    // Plain executable name: strip .exe and humanize
    if let Some(stem) = raw
        .strip_suffix(".exe")
        .or_else(|| raw.strip_suffix(".EXE"))
    {
        return humanize_identifier(stem);
    }

    humanize_identifier(raw)
}

fn humanize_identifier(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_is_lower = false;
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch == '.' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            prev_is_lower = false;
        } else if ch.is_uppercase() {
            if prev_is_lower && !out.ends_with(' ') {
                out.push(' ');
            }
            out.push(ch);
            prev_is_lower = false;
        } else {
            out.push(ch);
            prev_is_lower = ch.is_lowercase();
        }
    }
    // Capitalize first letter of each word if all lowercase
    let words: Vec<String> = out
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect();
    words.join(" ")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaProperties {
    pub title: String,
    pub artist: String,
    pub album_title: String,
    pub album_artist: String,
    pub track_number: Option<i32>,
    pub genres: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackStatus {
    #[default]
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRepeatMode {
    None,
    Track,
    List,
}

/// Tile inputs this integration serves: (value, icon, color, mode), the
/// same shape `pulpit_spotify::input_declarations` returns. The legacy
/// mapper resolves tile styling from these; the virtual-media-key
/// "Multimedia" kind stays untouched.
pub fn input_declarations() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    vec![
        ("media-now-playing", "music", "#8E44AD", Some("status")),
        ("media-control", "play", "#8E44AD", None),
        ("media-seek", "clock", "#8E44AD", Some("slider")),
    ]
}

/// Does this action kind belong to the system media integration? Exact
/// matches only, like `pulpit_spotify::is_spotify_action`.
pub fn is_media_action(kind: &str) -> bool {
    matches!(kind, "media-now-playing" | "media-control" | "media-seek")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlaybackInfo {
    pub status: PlaybackStatus,
    pub is_playing: bool,
    pub shuffle: Option<bool>,
    pub repeat: Option<AutoRepeatMode>,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_skip_next: bool,
    pub can_skip_prev: bool,
    pub can_stop: bool,
    pub can_seek: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimelineProperties {
    pub position_ms: u64,
    pub duration_ms: u64,
    pub min_seek_ms: u64,
    pub max_seek_ms: u64,
}

/// The internal host-consumed keys carrying thumbnail bytes and file
/// extension. Internal: stripped before any client forward, never rendered.
pub const ART_DATA_KEY: &str = "media-art-data";
pub const ART_EXT_KEY: &str = "media-art-ext";
/// Internal digest of the raw thumbnail bytes (hex SHA-256). The per-poll
/// change compare uses this instead of the base64 art, and the host lane
/// memoizes imported covers by it. Internal like the two keys above:
/// stripped before any client forward.
pub const ART_KEY_KEY: &str = "media-art-key";

/// Hex SHA-256 of the raw thumbnail bytes: the value of [`ART_KEY_KEY`].
pub fn art_digest(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(bytes))
}

/// Attach the thumbnail bytes (base64) and extension to a snapshot built
/// by [`build_snapshot_keyed`]. Kept separate so the poll loop encodes a
/// cover only when it is about to send it.
pub fn attach_art_data(snapshot: &mut Value, bytes: &[u8], ext: &str) {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    snapshot[ART_DATA_KEY] = Value::String(b64);
    snapshot[ART_EXT_KEY] = Value::String(ext.to_string());
}

/// The media card's corner-brand source (see the desktop/mobile media
/// renderers): the brand mark only renders for apps we carry a glyph
/// for, so an `App` name that is not Spotify yields no provider at all
/// and clients draw no mark - a YouTube-in-Chrome session must never
/// wear Spotify's logo.
fn corner_provider(app_name: &str) -> Option<&'static str> {
    app_name
        .to_lowercase()
        .contains("spotify")
        .then_some("spotify")
}

/// Build a snapshot payload matching the design §4 contract and
/// spotify-now-playing shape so desktop and mobile renderers are shared.
/// The cover, when given, travels as digest + base64 bytes + extension.
pub fn build_snapshot(
    props: Option<&MediaProperties>,
    playback: Option<&PlaybackInfo>,
    timeline: Option<&TimelineProperties>,
    app_name: &str,
    art: Option<(&[u8], &'static str)>,
) -> Value {
    let digest = art.map(|(bytes, _)| art_digest(bytes));
    let mut snapshot = build_snapshot_keyed(props, playback, timeline, app_name, digest.as_deref());
    if let Some((bytes, ext)) = art {
        attach_art_data(&mut snapshot, bytes, ext);
    }
    snapshot
}

/// [`build_snapshot`] without art bytes: only the cover's digest
/// ([`ART_KEY_KEY`]) rides along, so serializing the snapshot for the
/// change compare stays cheap. Add the bytes with [`attach_art_data`].
pub fn build_snapshot_keyed(
    props: Option<&MediaProperties>,
    playback: Option<&PlaybackInfo>,
    timeline: Option<&TimelineProperties>,
    app_name: &str,
    art_key: Option<&str>,
) -> Value {
    let is_playing = playback.map(|p| p.is_playing).unwrap_or(false);
    let shuffle = playback.and_then(|p| p.shuffle).unwrap_or(false);
    let repeat = match playback.and_then(|p| p.repeat) {
        Some(AutoRepeatMode::Track) => "track",
        Some(AutoRepeatMode::List) => "list",
        _ => "off",
    };
    let progress = match timeline {
        Some(t) if t.duration_ms > 0 => {
            format!(
                "{:.3}",
                (t.position_ms as f64 / t.duration_ms as f64).clamp(0.0, 1.0)
            )
        }
        _ => "0.000".to_string(),
    };

    let mut snapshot = json!({
        "media-playing": if is_playing { "ON" } else { "OFF" },
        "media-shuffle": if shuffle { "ON" } else { "OFF" },
        "media-repeat": repeat,
        "media-repeat-on": if repeat != "off" { "ON" } else { "OFF" },
        "media-progress": progress,
        "media-app": app_name,
    });

    if let Some(key) = art_key {
        snapshot[ART_KEY_KEY] = Value::String(key.to_string());
    }

    let mut now_playing = json!({
        "title": if !app_name.is_empty() { app_name } else { pulpit_i18n::media_idle_title() },
        "compact": "",
        "summary": "",
        "rows": [],
    });

    // Brand the payload's source when recognized (renderers gate their
    // corner mark on it; unknown brands stay unmarked).
    if let Some(provider) = corner_provider(app_name) {
        now_playing["provider"] = Value::String(provider.to_string());
    }

    let has_content = props.is_some_and(|p| !p.title.is_empty() || !p.artist.is_empty());
    if !has_content {
        let idle = pulpit_i18n::media_idle();
        now_playing["rows"] = json!([{ "label": "Status", "value": idle }]);
        now_playing["compact"] = json!(idle);
        now_playing["summary"] = json!(idle);
    } else {
        let p = props.unwrap();
        let track = p.title.clone();
        let artist = p.artist.clone();
        let mut rows = Vec::new();
        if !track.is_empty() {
            rows.push(json!({ "label": "Track", "value": track }));
        }
        if !artist.is_empty() {
            rows.push(json!({ "label": "Artist", "value": artist }));
        }
        if !p.album_title.is_empty() {
            rows.push(json!({ "label": "Album", "value": p.album_title }));
        }
        if !app_name.is_empty() {
            rows.push(json!({ "label": "App", "value": app_name }));
        }

        let compact = if !track.is_empty() && !artist.is_empty() {
            format!("{track} \u{2014} {artist}")
        } else if !track.is_empty() {
            track
        } else {
            artist
        };
        now_playing["compact"] = json!(compact);
        now_playing["summary"] = json!(compact);
        now_playing["rows"] = Value::Array(rows);

        if let Some(t) = timeline {
            if t.duration_ms > 0 {
                now_playing["progress"] = json!({
                    "position_ms": t.position_ms,
                    "duration_ms": t.duration_ms,
                    "playing": is_playing,
                });
            }
        }
    }

    snapshot["media-now-playing"] = now_playing;
    snapshot
}

/// Longest a playing track may go without a push: the resync heartbeat
/// that bounds the clients' local position extrapolation drift.
pub const POSITION_HEARTBEAT: std::time::Duration = std::time::Duration::from_secs(10);

/// The snapshot with its volatile, position-derived keys normalized out:
/// `media-progress` (a 3-decimal fraction of the position) and
/// `media-now-playing.progress.position_ms`. Everything else - titles,
/// play state, shuffle/repeat, `duration_ms`, the art digest - is semantic
/// and stays, so two snapshots of one playing track at different positions
/// share an identity.
pub fn identity_string(snapshot: &Value) -> String {
    let mut identity = snapshot.clone();
    if let Some(map) = identity.as_object_mut() {
        map.remove("media-progress");
    }
    if let Some(progress) = identity
        .get_mut("media-now-playing")
        .and_then(|np| np.get_mut("progress"))
        .and_then(Value::as_object_mut)
    {
        progress.remove("position_ms");
    }
    identity.to_string()
}

/// Outcome of [`PushGate::decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushDecision {
    /// Push this snapshot to the receiver.
    pub send: bool,
    /// A semantic field changed since the last push (not just the
    /// position). The late-cover follow-up keys on this.
    pub identity_changed: bool,
}

/// Position dead-band for the SMTC push: clients interpolate a playing
/// track's position locally, so a poll that only moved the position is not
/// worth a fan-out. Sends on any semantic change, on any change while
/// paused (a paused position move is a user-visible seek), and on a
/// [`POSITION_HEARTBEAT`] resync while playing; skips the rest.
#[derive(Debug, Default)]
pub struct PushGate {
    last_identity: Option<String>,
    last_full: String,
    last_send: Option<std::time::Instant>,
}

impl PushGate {
    pub fn decide(&mut self, snapshot: &Value, now: std::time::Instant) -> PushDecision {
        let identity = identity_string(snapshot);
        let identity_changed = self.last_identity.as_deref() != Some(identity.as_str());
        let paused = snapshot["media-playing"] != "ON";
        let full = if paused {
            snapshot.to_string()
        } else {
            String::new()
        };
        // The heartbeat resyncs extrapolating clients; a paused track
        // does not extrapolate, so it needs none.
        let heartbeat_due = !paused
            && self
                .last_send
                .is_none_or(|at| now.duration_since(at) >= POSITION_HEARTBEAT);
        // Paused polls repeat the same position until a seek moves it:
        // push only when the whole snapshot differs, not every tick.
        let paused_moved = paused && full != self.last_full;
        let send = identity_changed || paused_moved || heartbeat_due;
        if send {
            self.last_identity = Some(identity);
            self.last_full = full;
            self.last_send = Some(now);
        }
        PushDecision {
            send,
            identity_changed,
        }
    }
}

/// Execute a media playback command against the active or target session.
pub fn control(target_app: Option<&str>, action: &str, slider_value: Option<f64>) -> Result<bool> {
    #[cfg(windows)]
    {
        win_media::control(target_app, action, slider_value)
    }
    #[cfg(not(windows))]
    {
        let _ = (target_app, action, slider_value);
        Err(OsError::Unsupported("media"))
    }
}

/// List active media session titles (friendly app names).
pub fn list_sessions() -> Result<Vec<String>> {
    #[cfg(windows)]
    {
        win_media::list_sessions()
    }
    #[cfg(not(windows))]
    {
        Err(OsError::Unsupported("media"))
    }
}

/// Spawn the SMTC push poller / event loop.
pub fn spawn_push(
    consumers: Arc<dyn Fn() -> usize + Send + Sync + 'static>,
) -> tokio::sync::mpsc::UnboundedReceiver<Value> {
    #[cfg(windows)]
    {
        win_media::spawn_push(consumers)
    }
    #[cfg(not(windows))]
    {
        let _ = consumers;
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aumid_friendly_name_mapping_examples() {
        assert_eq!(aumid_to_friendly_name("Spotify.exe"), "Spotify");
        assert_eq!(aumid_to_friendly_name("spotify.exe"), "Spotify");
        assert_eq!(
            aumid_to_friendly_name("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"),
            "Spotify"
        );
        assert_eq!(aumid_to_friendly_name("chrome.exe"), "Google Chrome");
        assert_eq!(aumid_to_friendly_name("msedge.exe"), "Microsoft Edge");
        assert_eq!(aumid_to_friendly_name("firefox.exe"), "Firefox");
        assert_eq!(aumid_to_friendly_name("vlc.exe"), "VLC");
        assert_eq!(aumid_to_friendly_name("foobar2000.exe"), "foobar2000");
        assert_eq!(aumid_to_friendly_name("Tidal.exe"), "TIDAL");
        assert_eq!(aumid_to_friendly_name("AppleMusic.exe"), "Apple Music");
        assert_eq!(
            aumid_to_friendly_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"),
            "Media Player"
        );
        assert_eq!(aumid_to_friendly_name("custom_player.exe"), "Custom Player");
        assert_eq!(aumid_to_friendly_name("F0DC299D809B9700"), "Zen Browser");
    }

    // The payload-text locale is process-global (pulpit-i18n) and cargo
    // runs a crate's tests on a thread pool: locale-dependent tests
    // serialize on this guard, each pinning the locale it asserts under
    // and restoring the default afterwards.
    static LOCALE_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn pin_locale(locale: &str) -> std::sync::MutexGuard<'static, ()> {
        let guard = LOCALE_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pulpit_i18n::set_locale(locale);
        guard
    }

    #[test]
    fn build_snapshot_idle_shape() {
        let guard = pin_locale("pl");
        let snapshot = build_snapshot(None, None, None, "", None);
        assert_eq!(snapshot["media-playing"], "OFF");
        assert_eq!(snapshot["media-progress"], "0.000");
        assert_eq!(snapshot["media-repeat"], "off");
        assert_eq!(snapshot["media-repeat-on"], "OFF");
        assert_eq!(snapshot["media-shuffle"], "OFF");
        assert_eq!(snapshot["media-app"], "");
        let np = &snapshot["media-now-playing"];
        assert_eq!(np["title"], "Multimedia");
        assert_eq!(np["compact"], "Nic nie jest odtwarzane");
        assert_eq!(np["summary"], "Nic nie jest odtwarzane");
        assert_eq!(np["rows"][0]["label"], "Status");
        assert_eq!(np["rows"][0]["value"], "Nic nie jest odtwarzane");
        assert!(np.get("progress").is_none());
        assert!(np.get("provider").is_none());
        assert!(snapshot.get(ART_DATA_KEY).is_none());
        assert!(snapshot.get(ART_KEY_KEY).is_none());
        pulpit_i18n::reset_locale();
        drop(guard);
    }

    #[test]
    fn build_snapshot_idle_shape_english() {
        let guard = pin_locale("en");
        let snapshot = build_snapshot(None, None, None, "", None);
        let np = &snapshot["media-now-playing"];
        assert_eq!(np["title"], "Media");
        assert_eq!(np["compact"], "Nothing is playing");
        assert_eq!(np["summary"], "Nothing is playing");
        assert_eq!(np["rows"][0]["label"], "Status");
        assert_eq!(np["rows"][0]["value"], "Nothing is playing");
        // a known app still names itself in the title; the idle text
        // stays the payload-text locale's
        let with_app = build_snapshot(None, None, None, "Spotify", None);
        assert_eq!(with_app["media-now-playing"]["title"], "Spotify");
        assert_eq!(
            with_app["media-now-playing"]["compact"],
            "Nothing is playing"
        );
        pulpit_i18n::reset_locale();
        drop(guard);
    }

    #[test]
    fn locale_switch_changes_the_next_idle_payload() {
        let guard = pin_locale("en");
        let english =
            build_snapshot(None, None, None, "", None)["media-now-playing"]["compact"].clone();
        pulpit_i18n::set_locale("pl");
        let polish =
            build_snapshot(None, None, None, "", None)["media-now-playing"]["compact"].clone();
        assert_eq!(english, "Nothing is playing");
        assert_eq!(polish, "Nic nie jest odtwarzane");
        pulpit_i18n::reset_locale();
        drop(guard);
    }

    #[test]
    fn build_snapshot_playing_shape_with_art() {
        let props = MediaProperties {
            title: "Bohemian Rhapsody".into(),
            artist: "Queen".into(),
            album_title: "A Night at the Opera".into(),
            ..Default::default()
        };
        let playback = PlaybackInfo {
            status: PlaybackStatus::Playing,
            is_playing: true,
            shuffle: Some(true),
            repeat: Some(AutoRepeatMode::Track),
            can_play: true,
            can_pause: true,
            can_skip_next: true,
            can_skip_prev: true,
            can_stop: true,
            can_seek: true,
        };
        let timeline = TimelineProperties {
            position_ms: 60000,
            duration_ms: 354000,
            min_seek_ms: 0,
            max_seek_ms: 354000,
        };
        let art_bytes = b"fake-jpg-content";

        let snapshot = build_snapshot(
            Some(&props),
            Some(&playback),
            Some(&timeline),
            "Spotify",
            Some((art_bytes, "jpg")),
        );

        assert_eq!(snapshot["media-playing"], "ON");
        assert_eq!(snapshot["media-shuffle"], "ON");
        assert_eq!(snapshot["media-repeat"], "track");
        assert_eq!(snapshot["media-repeat-on"], "ON");
        assert_eq!(snapshot["media-progress"], "0.169");
        assert_eq!(snapshot["media-app"], "Spotify");
        assert!(snapshot.get(ART_DATA_KEY).is_some());
        assert_eq!(snapshot[ART_EXT_KEY], "jpg");
        assert_eq!(snapshot[ART_KEY_KEY], art_digest(art_bytes));

        let np = &snapshot["media-now-playing"];
        let prog = &np["progress"];
        assert_eq!(prog["position_ms"], 60000);
        assert_eq!(prog["duration_ms"], 354000);
        assert_eq!(prog["playing"], true);
        // the Spotify session is branded so the renderers' corner mark
        // shows; other apps carry no provider
        assert_eq!(np["provider"], "spotify");
        let chrome = build_snapshot(
            Some(&props),
            Some(&playback),
            Some(&timeline),
            "Google Chrome",
            None,
        );
        assert!(chrome["media-now-playing"].get("provider").is_none());
    }

    #[test]
    fn keyed_snapshot_carries_the_digest_but_never_the_art_bytes() {
        // The idle snapshot reads the process-global locale; pin it so a
        // concurrent locale test cannot swap the idle text between the
        // two builds this test compares.
        let _guard = pin_locale("en");
        let bytes = b"fake-jpg-content";
        let key = art_digest(bytes);
        assert_eq!(key.len(), 64);
        let keyed = build_snapshot_keyed(None, None, None, "Spotify", Some(&key));
        assert_eq!(keyed[ART_KEY_KEY], key.as_str());
        assert!(keyed.get(ART_DATA_KEY).is_none());
        assert!(keyed.get(ART_EXT_KEY).is_none());

        // no cover: no key either
        let bare = build_snapshot_keyed(None, None, None, "Spotify", None);
        assert!(bare.get(ART_KEY_KEY).is_none());

        // a different cover changes the compare string, the same one
        // does not (the stuck-cover guard rides on this)
        let other = art_digest(b"another-cover");
        let swapped = build_snapshot_keyed(None, None, None, "Spotify", Some(&other));
        assert_ne!(keyed.to_string(), swapped.to_string());
        let again = build_snapshot_keyed(None, None, None, "Spotify", Some(&key));
        assert_eq!(keyed.to_string(), again.to_string());

        // attaching afterwards yields the same payload build_snapshot does
        let mut attached = keyed.clone();
        attach_art_data(&mut attached, bytes, "jpg");
        let whole = build_snapshot(None, None, None, "Spotify", Some((bytes, "jpg")));
        assert_eq!(attached, whole);
    }

    fn gate_snapshot(title: &str, playing: bool, position_ms: u64, art: Option<&str>) -> Value {
        let props = MediaProperties {
            title: title.into(),
            artist: "Queen".into(),
            ..Default::default()
        };
        let playback = PlaybackInfo {
            is_playing: playing,
            ..Default::default()
        };
        let timeline = TimelineProperties {
            position_ms,
            duration_ms: 354000,
            ..Default::default()
        };
        build_snapshot_keyed(
            Some(&props),
            Some(&playback),
            Some(&timeline),
            "Spotify",
            art,
        )
    }

    #[test]
    fn identity_ignores_position_but_sees_semantic_changes() {
        let base = identity_string(&gate_snapshot("Song", true, 1000, Some("a")));
        // position moves: both position-derived keys are normalized away
        assert_eq!(
            base,
            identity_string(&gate_snapshot("Song", true, 61000, Some("a")))
        );
        assert_ne!(
            gate_snapshot("Song", true, 1000, Some("a")).to_string(),
            gate_snapshot("Song", true, 61000, Some("a")).to_string()
        );
        // art, title and play state are semantic
        assert_ne!(
            base,
            identity_string(&gate_snapshot("Song", true, 1000, Some("b")))
        );
        assert_ne!(
            base,
            identity_string(&gate_snapshot("Song", true, 1000, None))
        );
        assert_ne!(
            base,
            identity_string(&gate_snapshot("Other", true, 1000, Some("a")))
        );
        assert_ne!(
            base,
            identity_string(&gate_snapshot("Song", false, 1000, Some("a")))
        );
        // the duration stays in the identity
        let mut longer = gate_snapshot("Song", true, 1000, Some("a"));
        longer["media-now-playing"]["progress"]["duration_ms"] = json!(999);
        assert_ne!(base, identity_string(&longer));
    }

    #[test]
    fn push_gate_dead_bands_playing_position_only() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let mut gate = PushGate::default();

        // first snapshot always goes out
        let first = gate.decide(&gate_snapshot("Song", true, 1000, Some("a")), t0);
        assert!(first.send && first.identity_changed);

        // position-only polls inside the window are skipped
        let tick = gate.decide(
            &gate_snapshot("Song", true, 2000, Some("a")),
            t0 + Duration::from_secs(1),
        );
        assert!(!tick.send && !tick.identity_changed);
        let tick = gate.decide(
            &gate_snapshot("Song", true, 9000, Some("a")),
            t0 + Duration::from_secs(9),
        );
        assert!(!tick.send);

        // heartbeat resyncs the position after 10 s
        let beat = gate.decide(
            &gate_snapshot("Song", true, 10000, Some("a")),
            t0 + Duration::from_secs(10),
        );
        assert!(beat.send && !beat.identity_changed);
        let after = gate.decide(
            &gate_snapshot("Song", true, 11000, Some("a")),
            t0 + Duration::from_secs(11),
        );
        assert!(!after.send);

        // a late cover (art-only swap) and a title change send at once
        let art = gate.decide(
            &gate_snapshot("Song", true, 12000, Some("b")),
            t0 + Duration::from_secs(12),
        );
        assert!(art.send && art.identity_changed);
        let title = gate.decide(
            &gate_snapshot("Next", true, 0, Some("b")),
            t0 + Duration::from_secs(13),
        );
        assert!(title.send && title.identity_changed);
    }

    #[test]
    fn push_gate_sends_paused_position_moves_but_not_repeats() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let mut gate = PushGate::default();
        assert!(
            gate.decide(&gate_snapshot("Song", true, 5000, Some("a")), t0)
                .send
        );

        // pausing is a semantic change
        let pause = gate.decide(
            &gate_snapshot("Song", false, 5000, Some("a")),
            t0 + Duration::from_secs(1),
        );
        assert!(pause.send && pause.identity_changed);

        // the same paused snapshot polled again is not re-pushed...
        let same = gate.decide(
            &gate_snapshot("Song", false, 5000, Some("a")),
            t0 + Duration::from_secs(2),
        );
        assert!(!same.send);
        let long_idle = gate.decide(
            &gate_snapshot("Song", false, 5000, Some("a")),
            t0 + Duration::from_secs(60),
        );
        assert!(!long_idle.send);

        // ...but a seek while paused is
        let seek = gate.decide(
            &gate_snapshot("Song", false, 90000, Some("a")),
            t0 + Duration::from_secs(61),
        );
        assert!(seek.send && !seek.identity_changed);
    }

    #[test]
    fn input_declarations_cover_the_smtc_kinds_exactly() {
        for (kind, ..) in input_declarations() {
            assert!(is_media_action(kind), "{kind} must be claimable");
        }
        assert_eq!(input_declarations().len(), 3);
        // the virtual-media-key kind is NOT claimed by this integration
        assert!(!is_media_action("vol"));
        assert!(!is_media_action("media-unknown-future"));
    }

    #[test]
    #[ignore]
    fn live_smtc_smoke() {
        #[cfg(windows)]
        {
            let sessions = list_sessions().unwrap();
            println!("Live SMTC sessions: {sessions:?}");

            // the full push loop on real hardware: one snapshot must
            // arrive within a few seconds of someone watching
            let mut rx = spawn_push(std::sync::Arc::new(|| 1));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let snapshot = loop {
                match rx.try_recv() {
                    Ok(snapshot) => break snapshot,
                    Err(_) if std::time::Instant::now() > deadline => {
                        panic!("no snapshot within 10 s");
                    }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
                }
            };
            println!(
                "Live snapshot: playing={} app={:?} now-playing={}",
                snapshot["media-playing"],
                snapshot["media-app"].as_str().unwrap_or_default(),
                snapshot["media-now-playing"],
            );
        }
    }
}
