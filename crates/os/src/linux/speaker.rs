//! Default-playback control through `pactl` (works on PulseAudio and on
//! PipeWire through pipewire-pulse): volume, mute and sink switching of
//! `@DEFAULT_SINK@`. Output is parsed with pure functions below, tested
//! against real `LC_ALL=C pactl` output captured on PipeWire.

use crate::linux::{run_tool, TOOL_TIMEOUT};
use crate::{AudioDevice, OsError, Result, Speaker};

pub(crate) struct LinuxSpeaker;

const MISSING: &str = "speaker: pactl not found";

impl Speaker for LinuxSpeaker {
    fn volume(&mut self) -> Result<f32> {
        let out = run_tool(
            MISSING,
            "pactl",
            &["get-sink-volume", "@DEFAULT_SINK@"],
            None,
            TOOL_TIMEOUT,
        )?;
        parse_volume_percent(&String::from_utf8_lossy(&out))
    }

    fn muted(&mut self) -> Result<bool> {
        let out = run_tool(
            MISSING,
            "pactl",
            &["get-sink-mute", "@DEFAULT_SINK@"],
            None,
            TOOL_TIMEOUT,
        )?;
        parse_mute(&String::from_utf8_lossy(&out))
    }

    fn set_volume(&mut self, percent: f32) -> Result<()> {
        // whole percents only; pactl's own percent scale can exceed 100
        // (relative to the sink's base volume) but tiles clamp to 0..=100
        let rounded = percent.round().clamp(0.0, 100.0);
        let value = format!("{rounded:.0}%");
        run_tool(
            MISSING,
            "pactl",
            &["set-sink-volume", "@DEFAULT_SINK@", &value],
            None,
            TOOL_TIMEOUT,
        )
        .map(|_| ())
    }

    fn devices(&mut self) -> Result<Vec<AudioDevice>> {
        let default = self.active_device()?;
        let out = run_tool(
            MISSING,
            "pactl",
            &["-f", "json", "list", "sinks"],
            None,
            TOOL_TIMEOUT,
        )?;
        parse_sink_list(&String::from_utf8_lossy(&out), &default)
    }

    fn active_device(&mut self) -> Result<String> {
        let out = run_tool(MISSING, "pactl", &["get-default-sink"], None, TOOL_TIMEOUT)?;
        let name = String::from_utf8_lossy(&out).trim().to_string();
        if name.is_empty() {
            return Err(OsError::Failed("pactl: no default sink".into()));
        }
        Ok(name)
    }

    fn set_active_device(&mut self, id: &str) -> Result<()> {
        run_tool(
            MISSING,
            "pactl",
            &["set-default-sink", id],
            None,
            TOOL_TIMEOUT,
        )
        .map(|_| ())
    }
}

/// First `NN%` of `pactl get-sink-volume` output, clamped to 0..=100
/// (PipeWire sinks can sit above their base volume).
pub(crate) fn parse_volume_percent(output: &str) -> Result<f32> {
    let Some(percent_sign) = output.find('%') else {
        return Err(OsError::Failed(format!(
            "pactl volume output unparsable: {}",
            first_line(output)
        )));
    };
    let before = output[..percent_sign].trim_end();
    let digits_start = before
        .rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);
    before
        .get(digits_start..)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f32>().ok())
        .map(|v| v.clamp(0.0, 100.0))
        .ok_or_else(|| {
            OsError::Failed(format!(
                "pactl volume output unparsable: {}",
                first_line(output)
            ))
        })
}

/// `Mute: yes` / `Mute: no` of `pactl get-sink-mute`.
pub(crate) fn parse_mute(output: &str) -> Result<bool> {
    let trimmed = output.trim();
    if trimmed.ends_with("yes") {
        Ok(true)
    } else if trimmed.ends_with("no") {
        Ok(false)
    } else {
        Err(OsError::Failed(format!(
            "pactl mute output unparsable: {}",
            first_line(output)
        )))
    }
}

/// `pactl -f json list sinks`: the sink's `name` is the id tiles store
/// (the Windows endpoint-id equivalent), `description` the shown label.
pub(crate) fn parse_sink_list(json: &str, default_sink: &str) -> Result<Vec<AudioDevice>> {
    let sinks: serde_json::Value =
        serde_json::from_str(json).map_err(|e| OsError::Failed(format!("pactl sink list: {e}")))?;
    let Some(array) = sinks.as_array() else {
        return Err(OsError::Failed("pactl sink list: not a JSON array".into()));
    };
    Ok(array
        .iter()
        .map(|sink| {
            let name = sink.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = sink
                .get("description")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(name);
            AudioDevice {
                id: name.to_string(),
                name: description.to_string(),
                is_default: name == default_sink,
            }
        })
        .collect())
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // real `LC_ALL=C pactl` output captured on this machine (PipeWire)
    const VOLUME: &str = "Volume: front-left: 26214 /  40% / -23.88 dB,   front-right: 26214 /  40% / -23.88 dB\n        balance 0.00\n";

    #[test]
    fn volume_parser_takes_the_first_percent() {
        assert_eq!(parse_volume_percent(VOLUME).unwrap(), 40.0);
        assert_eq!(parse_volume_percent("Volume: 0%").unwrap(), 0.0);
        assert_eq!(parse_volume_percent("Volume: 100%").unwrap(), 100.0);
    }

    #[test]
    fn volume_above_one_hundred_is_reported_clamped() {
        // pactl scales percent against the sink's base volume; a boosted
        // sink reads 153% there. The tile contract is 0..=100.
        assert_eq!(
            parse_volume_percent("Volume: front-left: 39850 / 153% / 6.00 dB\n").unwrap(),
            100.0
        );
    }

    #[test]
    fn unparsable_volume_fails() {
        for bad in ["", "Volume: muted", "no percent here", "Volume: %"] {
            assert!(parse_volume_percent(bad).is_err(), "should fail: {bad:?}");
        }
    }

    #[test]
    fn mute_parser_distinguishes_states() {
        assert!(!parse_mute("Mute: no\n").unwrap());
        assert!(parse_mute("Mute: yes\n").unwrap());
        assert!(parse_mute("Mute: yes").is_ok());
        assert!(
            parse_mute("Wyciszono: tak\n").is_err(),
            "translated output must fail loudly"
        );
        assert!(parse_mute("").is_err());
    }

    const SINKS: &str = r#"[{"index":58,"state":"SUSPENDED","name":"alsa_output.pci-0000_00_1f.3.analog-stereo","description":"Built-in Audio Analog Stereo","driver":"PipeWire","mute":false,"volume":{"front-left":{"value":26214,"value_percent":"40%","db":"-23.88 dB"},"front-right":{"value":26214,"value_percent":"40%","db":"-23.88 dB"}}},{"index":59,"state":"RUNNING","name":"bluez_sink.11_22_33_44_55_66","description":"","driver":"PipeWire"}]"#;

    #[test]
    fn sink_list_maps_names_descriptions_and_default() {
        let devices = parse_sink_list(SINKS, "alsa_output.pci-0000_00_1f.3.analog-stereo").unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(
            devices[0],
            AudioDevice {
                id: "alsa_output.pci-0000_00_1f.3.analog-stereo".into(),
                name: "Built-in Audio Analog Stereo".into(),
                is_default: true,
            }
        );
        // missing description falls back to the raw name
        assert_eq!(devices[1].name, "bluez_sink.11_22_33_44_55_66");
        assert!(!devices[1].is_default);
    }

    #[test]
    fn sink_list_rejects_non_json() {
        assert!(parse_sink_list("not json", "x").is_err());
        assert!(parse_sink_list("{}", "x").is_err());
    }
}
