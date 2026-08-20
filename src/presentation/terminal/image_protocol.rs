//! Terminal image protocol support - probing and escape sequences for
//! iTerm2 and Kitty image display.

use base64::Engine;
use image::{GenericImageView, ImageFormat};
use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Read, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

pub(crate) const KITTY_IMAGE_ID: u32 = 1;
pub(crate) const KITTY_DELETE_ESCAPE: &str = "\x1b_Ga=d,d=I,i=1\x1b\\";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageProtocol {
    ITerm2,
    Kitty,
    None,
}

pub(crate) fn detect_image_protocol() -> ImageProtocol {
    #[cfg(unix)]
    {
        detect_image_protocol_unix()
    }
    #[cfg(not(unix))]
    {
        ImageProtocol::None
    }
}

#[cfg(unix)]
fn detect_image_protocol_unix() -> ImageProtocol {
    let Ok(mut tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
        return ImageProtocol::None;
    };

    drain_probe_replies(&mut tty);
    if run_probe(&mut tty, kitty_probe_query(), is_kitty_probe_response) {
        return ImageProtocol::Kitty;
    }

    drain_probe_replies(&mut tty);
    if run_probe(&mut tty, iterm_probe_query(), is_iterm_probe_response) {
        return ImageProtocol::ITerm2;
    }

    ImageProtocol::None
}

#[cfg(unix)]
fn run_probe(tty: &mut std::fs::File, query: &[u8], matches_response: fn(&[u8]) -> bool) -> bool {
    if tty.write_all(query).is_err() || tty.flush().is_err() {
        return false;
    }

    let deadline = Instant::now() + Duration::from_millis(120);
    let mut buf = [0u8; 1024];
    let mut reply = Vec::new();

    while wait_for_tty_input(tty, deadline) {
        let Ok(read) = tty.read(&mut buf) else {
            break;
        };
        if read == 0 {
            break;
        }
        reply.extend_from_slice(&buf[..read]);
        if matches_response(&reply) {
            return true;
        }
    }

    false
}

#[cfg(unix)]
fn drain_probe_replies(tty: &mut std::fs::File) {
    let deadline = Instant::now() + Duration::from_millis(10);
    let mut buf = [0u8; 512];
    while wait_for_tty_input(tty, deadline) {
        if tty.read(&mut buf).ok().filter(|read| *read > 0).is_none() {
            break;
        }
    }
}

#[cfg(unix)]
fn wait_for_tty_input(tty: &std::fs::File, deadline: Instant) -> bool {
    if Instant::now() >= deadline {
        return false;
    }

    let remaining = deadline.saturating_duration_since(Instant::now());
    let secs = remaining.as_secs().min(i64::from(i32::MAX) as u64) as libc::time_t;
    let micros = remaining.subsec_micros() as libc::suseconds_t;
    let fd = tty.as_raw_fd();

    let mut readfds = unsafe { std::mem::zeroed::<libc::fd_set>() };
    let mut timeout = libc::timeval {
        tv_sec: secs,
        tv_usec: micros,
    };

    unsafe {
        libc::FD_ZERO(&mut readfds);
        libc::FD_SET(fd, &mut readfds);
        libc::select(
            fd + 1,
            &mut readfds,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut timeout,
        ) > 0
    }
}

/// Builds a Kitty graphics escape for `bytes`, sizing it to fit the terminal box.
pub(crate) fn kitty_image_escape(bytes: &[u8], columns: u16, rows: u16) -> Option<String> {
    let image = image::load_from_memory(bytes).ok()?;
    let (width, height) = image.dimensions();
    let payload = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    } else {
        let mut png = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .ok()?;
        base64::engine::general_purpose::STANDARD.encode(png)
    };
    let control = kitty_size_control(width, height, columns, rows);
    Some(format!(
        "\x1b_Ga=T,f=100,i={KITTY_IMAGE_ID},{control},m=0;{payload}\x1b\\"
    ))
}

fn kitty_size_control(width: u32, height: u32, columns: u16, rows: u16) -> String {
    let box_aspect = columns as f32 / rows.max(1) as f32;
    let image_aspect = width as f32 / height.max(1) as f32;
    if image_aspect >= box_aspect {
        format!("c={columns}")
    } else {
        format!("r={rows}")
    }
}

/// Cheap identity hash so identical images aren't resent to Kitty.
pub(crate) fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn kitty_probe_query() -> &'static [u8] {
    b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c"
}

fn is_kitty_probe_response(bytes: &[u8]) -> bool {
    bytes
        .windows(b"\x1b_Gi=31;".len())
        .any(|window| window == b"\x1b_Gi=31;")
}

fn iterm_probe_query() -> &'static [u8] {
    b"\x1b]1337;ReportCellSize\x07"
}

fn is_iterm_probe_response(bytes: &[u8]) -> bool {
    bytes
        .windows(b"\x1b]1337;ReportCellSize=".len())
        .any(|window| window == b"\x1b]1337;ReportCellSize=")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn kitty_escape_uses_kitty_graphics_prefix() {
        let png = tiny_png();
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(esc.starts_with("\x1b_Ga=T,f=100,i=1,"));
        assert!(esc.ends_with("\x1b\\"));
    }

    #[test]
    fn kitty_escape_uses_width_only_for_wide_images() {
        let png = test_png(400, 100);
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(esc.contains(",c=40"));
        assert!(!esc.contains(",r=18"));
    }

    #[test]
    fn kitty_escape_uses_height_only_for_tall_images() {
        let png = test_png(100, 200);
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(esc.contains(",r=18"));
        assert!(!esc.contains(",c=40"));
    }

    #[test]
    fn kitty_escape_passes_png_bytes_through_without_reencoding() {
        let png = test_png(8, 8);
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(
            esc.contains(",r=18"),
            "dimensions are read from the PNG header"
        );
    }

    #[test]
    fn detects_kitty_probe_response() {
        assert!(is_kitty_probe_response(b"\x1b_Gi=31;OK\x1b\\"));
        assert!(!is_kitty_probe_response(b"\x1b[c"));
    }

    #[test]
    fn detects_iterm_probe_response() {
        assert!(is_iterm_probe_response(
            b"\x1b]1337;ReportCellSize=17.50;8.00;2.0\x07"
        ));
        assert!(!is_iterm_probe_response(b"\x1b]1337;CursorShape=1\x07"));
    }

    #[cfg(test)]
    pub(crate) fn tiny_png() -> Vec<u8> {
        test_png(1, 1)
    }

    #[cfg(test)]
    pub(crate) fn test_png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    /// A tiny animated GIF with `frames` frames of `delay_cs` centiseconds each.
    pub(crate) fn test_gif(frames: usize, delay_cs: u16) -> Vec<u8> {
        use image::codecs::gif::GifEncoder;
        let mut bytes = Vec::new();
        let mut encoder = GifEncoder::new(&mut bytes);
        for i in 0..frames {
            let img = image::RgbImage::from_fn(16, 16, |x, y| {
                image::Rgb([(x + i as u32) as u8, y as u8, 0])
            });
            let frame = image::Frame::from_parts(
                image::DynamicImage::ImageRgb8(img).to_rgba8(),
                0,
                0,
                image::Delay::from_numer_denom_ms(u32::from(delay_cs) * 10, 1),
            );
            encoder.encode_frame(frame).unwrap();
        }
        drop(encoder);
        bytes
    }
}
