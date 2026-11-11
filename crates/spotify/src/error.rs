//! Typed errors. `Display` of the three design-mapped variants is the
//! user-facing message (design §3): toasts on the desktop, the flash path
//! on tablets read exactly these strings.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpotifyError {
    /// `invalid_grant`, or no tokens at all: the saved login is gone or
    /// expired (refresh tokens reportedly live ~6 months). Never retried
    /// by the crate - the user must log in again from the settings.
    #[error("Log in to Spotify in Pulpit settings")]
    NeedsLogin,
    /// 403 with `reason: PREMIUM_REQUIRED`: control endpoints need
    /// Premium; reads work on Free.
    #[error("Spotify Premium required")]
    PremiumRequired,
    /// 404 with `reason: NO_ACTIVE_DEVICE`: nothing is playing anywhere.
    #[error("Open Spotify on a device first")]
    NoActiveDevice,
    /// 429 with `Retry-After`: the whole handle pauses for
    /// `Retry-After + 0.5 s`; every later call fails fast with the
    /// remaining pause.
    #[error("Spotify rate limit - retry in {retry_after_secs}s")]
    RateLimited { retry_after_secs: u32 },
    /// A real API answer the caller must see as a failure (non-2xx
    /// without a mapped reason). Carries Spotify's own message.
    #[error("Spotify error: {0}")]
    Api(String),
    /// Transport-level failure: the request never reached Spotify.
    /// Distinguished from a rejection so offline machines do not look
    /// like they need a login (same discipline as `DiscordError::Network`).
    #[error("network unreachable for Spotify ({0})")]
    Network(&'static str),
    /// The tile's command JSON could not be parsed into an action.
    #[error("bad tile payload for {0}: {1}")]
    BadPayload(&'static str, String),
    /// The login flow failed before tokens existed (port busy, callback
    /// denied/timed out, code exchange rejected).
    #[error("Spotify login failed: {0}")]
    Login(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SpotifyError>;

/// User-facing text for toasts/flashes: the design §3 messages are the
/// `Display` strings of the mapped variants, everything else keeps its
/// technical text (shown in logs). The three design messages follow the
/// UI language (pulpit-i18n process locale); the `Display` strings stay
/// English - they ride in logs.
pub fn user_message(err: &SpotifyError) -> String {
    match err {
        SpotifyError::NeedsLogin => pulpit_i18n::tr(
            "Log in to Spotify in Pulpit settings",
            "Zaloguj się do Spotify w ustawieniach Pulpit",
        )
        .to_string(),
        SpotifyError::PremiumRequired => {
            pulpit_i18n::tr("Spotify Premium required", "Wymagane Spotify Premium").to_string()
        }
        SpotifyError::NoActiveDevice => pulpit_i18n::tr(
            "Open Spotify on a device first",
            "Najpierw otwórz Spotify na jakimś urządzeniu",
        )
        .to_string(),
        _ => err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_messages_are_the_display_strings() {
        // The three design messages follow the process locale, and other
        // test modules in this crate pin "pl" under their own guard -
        // either language's exact sentence is correct here.
        assert!(matches!(
            user_message(&SpotifyError::NeedsLogin).as_str(),
            "Log in to Spotify in Pulpit settings" | "Zaloguj się do Spotify w ustawieniach Pulpit"
        ));
        assert!(matches!(
            user_message(&SpotifyError::PremiumRequired).as_str(),
            "Spotify Premium required" | "Wymagane Spotify Premium"
        ));
        assert!(matches!(
            user_message(&SpotifyError::NoActiveDevice).as_str(),
            "Open Spotify on a device first" | "Najpierw otwórz Spotify na jakimś urządzeniu"
        ));
    }

    #[test]
    fn technical_variants_keep_their_detail() {
        assert_eq!(
            SpotifyError::RateLimited {
                retry_after_secs: 7
            }
            .to_string(),
            "Spotify rate limit - retry in 7s"
        );
        assert_eq!(
            SpotifyError::Api("Invalid id".into()).to_string(),
            "Spotify error: Invalid id"
        );
        assert_eq!(
            SpotifyError::Network("dns").to_string(),
            "network unreachable for Spotify (dns)"
        );
        assert_eq!(
            SpotifyError::BadPayload("spotify-volume", "not json".into()).to_string(),
            "bad tile payload for spotify-volume: not json"
        );
        assert_eq!(
            SpotifyError::Login("port busy".into()).to_string(),
            "Spotify login failed: port busy"
        );
    }

    #[test]
    fn io_errors_convert_transparently() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "locked");
        let err: SpotifyError = io.into();
        assert!(matches!(err, SpotifyError::Io(_)));
        assert_eq!(err.to_string(), "locked");
    }
}
