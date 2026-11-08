//! Seasonal skin producer ("tryb świąteczny" and friends,
//! .scratch/christmas-mode).
//!
//! Decides which season is active and pushes it as two custom-value keys
//! through the regular producer lanes (`pulpit_host::forward_producer`):
//!
//! - `season`: `"christmas"`, `"halloween"`, `"easter"`, `"valentines"`
//!   or `"off"`, the effective season every surface skins itself from
//!   (`ext.season` on the v2 wire).
//! - `season-countdown`: the days left until Wigilia (24 December) as a
//!   display string, which any Variable Value tile shows, the stock
//!   Deckboard client included.
//!
//! The mode lives in `editor.json` under `"season"`: `auto` follows the
//! calendar (local time, see [`season_on`]), `off` never skins, and a
//! season's own name forces that season all year round.

use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate};
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};

/// The `editor.json` key holding the mode.
pub const SETTING_KEY: &str = "season";
/// Pushed key carrying the effective season.
pub const KEY_SEASON: &str = "season";
/// Pushed key carrying the days-until-Wigilia display text.
pub const KEY_COUNTDOWN: &str = "season-countdown";

/// How often the producer re-evaluates the calendar. Pushes are
/// change-gated downstream, so a tick that changes nothing costs nothing
/// on the wire; this only bounds how late the midnight rollover lands.
const TICK: Duration = Duration::from_secs(5 * 60);

/// The user's choice in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Follow the calendar.
    #[default]
    Auto,
    /// This season all year round.
    Force(Season),
    /// Never.
    Off,
}

impl Mode {
    /// Every stored value Settings can write, in display order.
    pub const ALL: [Mode; 6] = [
        Mode::Auto,
        Mode::Off,
        Mode::Force(Season::Christmas),
        Mode::Force(Season::Halloween),
        Mode::Force(Season::Easter),
        Mode::Force(Season::Valentines),
    ];

    /// Parse a stored string strictly: `None` for anything unknown.
    /// `"on"` is the first build's spelling of forced Christmas.
    pub fn from_name(s: &str) -> Option<Mode> {
        match s {
            "auto" => Some(Mode::Auto),
            "off" => Some(Mode::Off),
            "on" => Some(Mode::Force(Season::Christmas)),
            other => Season::from_wire(other).map(Mode::Force),
        }
    }

    /// Parse the stored string; anything unknown or missing is `Auto`.
    pub fn parse(s: Option<&str>) -> Mode {
        s.and_then(Mode::from_name).unwrap_or_default()
    }

    /// Read the mode out of a parsed `editor.json` (`Null` when the file
    /// is missing).
    pub fn from_editor_json(stored: &Value) -> Mode {
        Mode::parse(stored.get(SETTING_KEY).and_then(Value::as_str))
    }

    /// The stored string; a forced season is stored by its wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Auto => "auto",
            Mode::Off => "off",
            Mode::Force(season) => season.wire_name(),
        }
    }
}

/// The effective season.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Christmas,
    Halloween,
    Easter,
    Valentines,
    Off,
}

impl Season {
    /// The exact string pushed under [`KEY_SEASON`] and matched by every
    /// client.
    pub fn wire_name(self) -> &'static str {
        match self {
            Season::Christmas => "christmas",
            Season::Halloween => "halloween",
            Season::Easter => "easter",
            Season::Valentines => "valentines",
            Season::Off => "off",
        }
    }

    /// A real season by its wire name (`"off"` is not one).
    pub fn from_wire(s: &str) -> Option<Season> {
        match s {
            "christmas" => Some(Season::Christmas),
            "halloween" => Some(Season::Halloween),
            "easter" => Some(Season::Easter),
            "valentines" => Some(Season::Valentines),
            _ => None,
        }
    }
}

/// Whether `date` falls in the Christmas season: 1 December through
/// 6 January (Trzech Króli), both inclusive.
pub fn in_christmas_season(date: NaiveDate) -> bool {
    match date.month() {
        12 => true,
        1 => date.day() <= 6,
        _ => false,
    }
}

/// Easter Sunday of `year` (Gregorian, the anonymous computus - the
/// date Polish calendars print).
pub fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("computus yields a real date")
}

/// The calendar's season on `date`, local time, every window inclusive:
///
/// - Walentynki: 10 to 14 February,
/// - Wielkanoc: Palm Sunday to Easter Monday (Easter - 7 to Easter + 1),
/// - Halloween: 25 October to 1 November (Wszystkich Świętych),
/// - Boże Narodzenie: 1 December to 6 January (Trzech Króli).
///
/// The windows never overlap: Easter moves between 15 March and 26 April.
pub fn season_on(date: NaiveDate) -> Season {
    if in_christmas_season(date) {
        return Season::Christmas;
    }
    let (m, d) = (date.month(), date.day());
    if m == 2 && (10..=14).contains(&d) {
        return Season::Valentines;
    }
    if (m == 10 && d >= 25) || (m == 11 && d == 1) {
        return Season::Halloween;
    }
    let easter = easter_sunday(date.year());
    let from = easter - chrono::Duration::days(7);
    let to = easter + chrono::Duration::days(1);
    if (from..=to).contains(&date) {
        return Season::Easter;
    }
    Season::Off
}

/// The season `mode` resolves to on `date`.
pub fn resolve(mode: Mode, date: NaiveDate) -> Season {
    match mode {
        Mode::Force(season) => season,
        Mode::Off => Season::Off,
        Mode::Auto => season_on(date),
    }
}

/// Days from `date` to the next Wigilia (24 December): 0 on the day
/// itself, and counting towards next year's from the 25th on.
pub fn days_until_wigilia(date: NaiveDate) -> i64 {
    let this_year = NaiveDate::from_ymd_opt(date.year(), 12, 24).expect("24 December exists");
    let target = if date <= this_year {
        this_year
    } else {
        NaiveDate::from_ymd_opt(date.year() + 1, 12, 24).expect("24 December exists")
    };
    (target - date).num_days()
}

/// The pushed snapshot for `mode` on `date`. The countdown is a plain
/// display string ("46 dni" / "46 days", in the payload locale): Variable
/// Value tiles on every surface, the stock client included, render a
/// string verbatim and ignore objects.
pub fn snapshot(mode: Mode, date: NaiveDate) -> Value {
    json!({
        KEY_SEASON: resolve(mode, date).wire_name(),
        KEY_COUNTDOWN: pulpit_i18n::season_countdown(days_until_wigilia(date)),
    })
}

/// Tile kinds this source declares, `(value, icon, color, mode)` like
/// the other native sources: the countdown is a custom-value display, so
/// the editor, the stock client and the v2 Toggle all show the pushed
/// `season-countdown` text with their existing renderers.
pub fn input_declarations() -> Vec<(&'static str, &'static str, &'static str, Option<&'static str>)> {
    vec![(KEY_COUNTDOWN, "gifts", "#2f7d4a", Some("custom-value"))]
}

/// Today in local time.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Start the producer: one snapshot right away, then one per [`TICK`]
/// and one immediately after every mode change. Needs a tokio runtime.
/// A dropped mode sender freezes the mode (the calendar keeps ticking);
/// the task ends when the push receiver is dropped.
pub fn spawn_push(mut mode: watch::Receiver<Mode>) -> mpsc::UnboundedReceiver<Value> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut mode_live = true;
        loop {
            let current = *mode.borrow_and_update();
            if tx.send(snapshot(current, today())).is_err() {
                tracing::debug!("season push channel closed, stopping");
                return;
            }
            tokio::select! {
                changed = mode.changed(), if mode_live => {
                    mode_live = changed.is_ok();
                }
                _ = tokio::time::sleep(TICK) => {}
            }
        }
    });
    rx
}

/// [`spawn_push`] for a host with no way to change the mode at runtime
/// (the headless server).
pub fn spawn_push_fixed(mode: Mode) -> mpsc::UnboundedReceiver<Value> {
    let (_tx, rx) = watch::channel(mode);
    spawn_push(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn season_runs_from_first_december_to_epiphany() {
        assert!(!in_christmas_season(d(2026, 11, 30)));
        assert!(in_christmas_season(d(2026, 12, 1)));
        assert!(in_christmas_season(d(2026, 12, 31)));
        assert!(in_christmas_season(d(2027, 1, 6)));
        assert!(!in_christmas_season(d(2027, 1, 7)));
        assert!(!in_christmas_season(d(2027, 7, 1)));
    }

    #[test]
    fn mode_overrides_the_calendar() {
        let summer = d(2026, 7, 1);
        let december = d(2026, 12, 10);
        assert_eq!(resolve(Mode::Auto, summer), Season::Off);
        assert_eq!(resolve(Mode::Auto, december), Season::Christmas);
        assert_eq!(resolve(Mode::Force(Season::Christmas), summer), Season::Christmas);
        assert_eq!(resolve(Mode::Force(Season::Halloween), december), Season::Halloween);
        assert_eq!(resolve(Mode::Off, december), Season::Off);
    }

    #[test]
    fn computus_matches_the_printed_calendar() {
        assert_eq!(easter_sunday(2024), d(2024, 3, 31));
        assert_eq!(easter_sunday(2025), d(2025, 4, 20));
        assert_eq!(easter_sunday(2026), d(2026, 4, 5));
        assert_eq!(easter_sunday(2027), d(2027, 3, 28));
        assert_eq!(easter_sunday(2038), d(2038, 4, 25)); // latest possible
        assert_eq!(easter_sunday(2285), d(2285, 3, 22)); // earliest possible
    }

    #[test]
    fn calendar_windows_for_every_season() {
        assert_eq!(season_on(d(2027, 2, 9)), Season::Off);
        assert_eq!(season_on(d(2027, 2, 10)), Season::Valentines);
        assert_eq!(season_on(d(2027, 2, 14)), Season::Valentines);
        assert_eq!(season_on(d(2027, 2, 15)), Season::Off);
        // Easter 2026: 5 April -> Palm Sunday 29 March .. Easter Monday 6 April
        assert_eq!(season_on(d(2026, 3, 28)), Season::Off);
        assert_eq!(season_on(d(2026, 3, 29)), Season::Easter);
        assert_eq!(season_on(d(2026, 4, 6)), Season::Easter);
        assert_eq!(season_on(d(2026, 4, 7)), Season::Off);
        assert_eq!(season_on(d(2026, 10, 24)), Season::Off);
        assert_eq!(season_on(d(2026, 10, 25)), Season::Halloween);
        assert_eq!(season_on(d(2026, 11, 1)), Season::Halloween);
        assert_eq!(season_on(d(2026, 11, 2)), Season::Off);
        assert_eq!(season_on(d(2026, 12, 1)), Season::Christmas);
        assert_eq!(season_on(d(2027, 1, 6)), Season::Christmas);
    }

    #[test]
    fn unknown_or_missing_mode_is_auto() {
        assert_eq!(Mode::parse(None), Mode::Auto);
        assert_eq!(Mode::parse(Some("bogus")), Mode::Auto);
        assert_eq!(Mode::parse(Some("on")), Mode::Force(Season::Christmas));
        assert_eq!(Mode::parse(Some("off")), Mode::Off);
        assert_eq!(Mode::parse(Some("easter")), Mode::Force(Season::Easter));
        assert_eq!(Mode::from_name("bogus"), None);
        assert_eq!(Mode::from_editor_json(&Value::Null), Mode::Auto);
        assert_eq!(Mode::from_editor_json(&json!({"season": "off"})), Mode::Off);
        for mode in Mode::ALL {
            assert_eq!(Mode::parse(Some(mode.as_str())), mode);
        }
    }

    #[test]
    fn countdown_targets_the_next_wigilia() {
        assert_eq!(days_until_wigilia(d(2026, 12, 23)), 1);
        assert_eq!(days_until_wigilia(d(2026, 12, 24)), 0);
        assert_eq!(days_until_wigilia(d(2026, 12, 25)), 364);
        assert_eq!(days_until_wigilia(d(2026, 11, 8)), 46);
        // leap year: 25 Dec 2027 -> 24 Dec 2028 spans 29 February
        assert_eq!(days_until_wigilia(d(2027, 12, 25)), 365);
    }

    #[test]
    fn snapshot_carries_both_keys() {
        let snap = snapshot(Mode::Auto, d(2026, 12, 20));
        assert_eq!(snap["season"], "christmas");
        // the payload locale is process-global; this crate never sets it,
        // so it is the English default here
        assert_eq!(snap["season-countdown"], "4 days");
        assert_eq!(snapshot(Mode::Off, d(2026, 12, 20))["season"], "off");
    }

    #[tokio::test]
    async fn mode_change_pushes_immediately() {
        let (tx, rx) = watch::channel(Mode::Off);
        let mut pushes = spawn_push(rx);
        let first = pushes.recv().await.unwrap();
        assert_eq!(first["season"], "off");
        tx.send(Mode::Force(Season::Christmas)).unwrap();
        let second = tokio::time::timeout(Duration::from_secs(2), pushes.recv())
            .await
            .expect("push within the timeout")
            .unwrap();
        assert_eq!(second["season"], "christmas");
    }

    #[tokio::test(start_paused = true)]
    async fn dropped_mode_sender_keeps_ticking() {
        let mut pushes = spawn_push_fixed(Mode::Force(Season::Christmas));
        assert_eq!(pushes.recv().await.unwrap()["season"], "christmas");
        // paused clock: the next recv auto-advances to the tick
        assert_eq!(pushes.recv().await.unwrap()["season"], "christmas");
    }
}
