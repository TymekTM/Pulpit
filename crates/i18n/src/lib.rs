//! Process-global locale for display-ready payload texts.
//!
//! Status payloads carry display-ready strings (the stock Deckboard
//! client renders values verbatim - the wire cannot carry keys), so the
//! media and Spotify producers localize their user-facing texts at
//! snapshot build time. The locale is process-global and chosen by the
//! host: the desktop mirrors its UI language (the frontend resolves
//! `auto` - only it knows the system locale), the headless server reads
//! the `lang` key of settings.json. Unknown locales fall back to
//! English; scope is ONLY these user-facing texts (sysinfo / aidev /
//! tools titles stay English).

use std::sync::RwLock;

/// Empty until [`set_locale`]; [`locale`] maps empty (and a poisoned
/// lock) back to the default so a fresh process is English.
static LOCALE: RwLock<String> = RwLock::new(String::new());

/// The process locale: `"en"` until [`set_locale`].
pub fn locale() -> String {
    match LOCALE.read() {
        Ok(stored) if !stored.is_empty() => stored.clone(),
        _ => "en".to_string(),
    }
}

/// Set the payload-text locale. An unknown code is stored verbatim and
/// every helper falls back to English for it.
pub fn set_locale(locale: &str) {
    if let Ok(mut stored) = LOCALE.write() {
        *stored = locale.to_string();
    }
}

/// Back to the default (`"en"`). Tests need this - `cargo test` runs a
/// crate's suite on a thread pool and the locale is process-global, so
/// a test that switches the locale must pin and restore it.
pub fn reset_locale() {
    set_locale("");
}

/// Polish when the locale says so, English otherwise (any unknown
/// locale included).
fn pick(en: &'static str, pl: &'static str) -> &'static str {
    if locale() == "pl" {
        pl
    } else {
        en
    }
}

/// The media card's fallback title: what `media-now-playing.title`
/// carries when no source app is known.
pub fn media_idle_title() -> &'static str {
    pick("Media", "Multimedia")
}

/// The system-media idle text (the Status row value, compact and
/// summary) for when nothing is playing.
pub fn media_idle() -> &'static str {
    pick("Nothing is playing", "Nic nie jest odtwarzane")
}

/// The Spotify idle text (the Status row value, compact and summary)
/// for when nothing is playing.
pub fn spotify_idle() -> &'static str {
    pick("Nothing is playing", "Nic nie jest odtwarzane")
}

/// The Spotify needs-login hint (rows value, compact and summary).
pub fn spotify_login_hint() -> &'static str {
    pick(
        "Log in via Pulpit settings",
        "Zaloguj się w ustawieniach Pulpita",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The locale is process-global and cargo runs a crate's tests on a
    // thread pool: locale-dependent tests serialize on this guard and
    // each pins the locale it asserts under. into_inner keeps one
    // failing test from poisoning the rest.
    static LOCALE_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn pin() -> std::sync::MutexGuard<'static, ()> {
        LOCALE_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn default_locale_is_english() {
        let _guard = pin();
        reset_locale();
        assert_eq!(locale(), "en");
        assert_eq!(media_idle_title(), "Media");
        assert_eq!(media_idle(), "Nothing is playing");
        assert_eq!(spotify_idle(), "Nothing is playing");
        assert_eq!(spotify_login_hint(), "Log in via Pulpit settings");
    }

    #[test]
    fn polish_locale_switches_every_helper() {
        let _guard = pin();
        set_locale("pl");
        assert_eq!(locale(), "pl");
        assert_eq!(media_idle_title(), "Multimedia");
        assert_eq!(media_idle(), "Nic nie jest odtwarzane");
        assert_eq!(spotify_idle(), "Nic nie jest odtwarzane");
        assert_eq!(spotify_login_hint(), "Zaloguj się w ustawieniach Pulpita");
        reset_locale();
    }

    #[test]
    fn unknown_locale_falls_back_to_english() {
        let _guard = pin();
        set_locale("de");
        assert_eq!(locale(), "de");
        assert_eq!(media_idle_title(), "Media");
        assert_eq!(media_idle(), "Nothing is playing");
        assert_eq!(spotify_idle(), "Nothing is playing");
        assert_eq!(spotify_login_hint(), "Log in via Pulpit settings");
        reset_locale();
    }

    #[test]
    fn reset_returns_to_the_default() {
        let _guard = pin();
        set_locale("pl");
        reset_locale();
        assert_eq!(locale(), "en");
        assert_eq!(media_idle(), "Nothing is playing");
    }
}
