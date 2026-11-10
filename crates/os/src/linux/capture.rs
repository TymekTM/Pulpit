//! Screen capture through the session's screenshot tool: `grim` on
//! Wayland, ImageMagick's `import` on X11. Both write a PNG to stdout.

use std::time::Duration;

use crate::linux::clipboard::{session_backend, SessionBackend};
use crate::linux::run_tool;
use crate::{OsError, Result};

/// Screenshots take longer than a volume query (a compositor round
/// trip, PNG encode); the runner still needs a hard bound.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

const NO_SESSION: &str = "screen capture: no display session";

pub(crate) fn grab_screen_rgba() -> Result<image::RgbaImage> {
    let backend = session_backend(
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
    .ok_or(OsError::Unsupported(NO_SESSION))?;
    let (program, args, missing): (&str, &[&str], &'static str) = match backend {
        SessionBackend::Wayland => ("grim", &["-"], "screen capture: grim not found"),
        SessionBackend::X11 => (
            "import",
            &["-window", "root", "png:-"],
            "screen capture: import not found",
        ),
    };
    let png = run_tool(missing, program, args, None, CAPTURE_TIMEOUT)?;
    decode_png_rgba(&png)
}

/// Decode stdout PNG bytes into RGBA. Split out so the decode path is
/// testable without a session (the tool invocation is not).
pub(crate) fn decode_png_rgba(bytes: &[u8]) -> Result<image::RgbaImage> {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map(|image| image.to_rgba8())
        .map_err(|e| OsError::Failed(format!("decode screenshot: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_bytes_round_trip_through_the_decode_helper() {
        // a tiny in-test PNG through the same encoder the atomic writer
        // uses, then back through the decode helper
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.png");
        let rgba = image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
        image::save_buffer_with_format(
            &path,
            rgba.as_raw(),
            3,
            2,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();

        let decoded = decode_png_rgba(&bytes).expect("decode");
        assert_eq!(decoded.dimensions(), (3, 2));
        assert_eq!(decoded.get_pixel(2, 1), &image::Rgba([10, 20, 30, 255]));
    }

    #[test]
    fn non_png_bytes_fail_the_decode() {
        assert!(decode_png_rgba(b"not a png at all").is_err());
        assert!(decode_png_rgba(&[]).is_err());
    }
}
