//! Helper for rendering RAW/raster inputs via `jpgfromrawlib` so quick-view stays on the existing image path.

use jpgfromrawlib::{FindJpegType, SUPPORTED_EXTENSIONS, process_file_bytes};
use once_cell::sync::OnceCell;
use std::path::Path;
use tokio::runtime::{Builder, Runtime};

const FIND_TYPE: FindJpegType = FindJpegType::Largest;
type ProcessBytesFn = fn(&Path) -> Option<Vec<u8>>;

pub fn is_supported_ext(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS
        .iter()
        .any(|supported| ext.eq_ignore_ascii_case(supported))
}

pub fn supports_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(is_supported_ext)
        .unwrap_or(false)
}

/// Attempts to produce JPEG bytes for supported RAW/raster files.
/// If the extension isn't listed in `SUPPORTED_EXTENSIONS`, returns `None`.
pub fn try_render_from_raw(path: &Path) -> Option<Vec<u8>> {
    try_render_with(path, call_process_file_bytes)
}

fn try_render_with(path: &Path, processor: ProcessBytesFn) -> Option<Vec<u8>> {
    if !supports_path(path) {
        return None;
    }

    processor(path)
}

#[cfg(test)]
pub(crate) fn try_render_with_processor(path: &Path, processor: ProcessBytesFn) -> Option<Vec<u8>> {
    try_render_with(path, processor)
}

fn call_process_file_bytes(path: &Path) -> Option<Vec<u8>> {
    runtime().block_on(process_file_bytes(path, FIND_TYPE)).ok()
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceCell<Runtime> = OnceCell::new();
    RUNTIME.get_or_init(|| {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn stub_jpeg(_: &Path) -> Option<Vec<u8>> {
        Some(vec![0xFF, 0xD8])
    }

    fn stub_single_byte(_: &Path) -> Option<Vec<u8>> {
        Some(vec![0xFF])
    }

    #[test]
    fn returns_none_for_unsupported_extension() {
        let path = PathBuf::from("photo.txt");
        assert!(try_render_from_raw(&path).is_none());
    }

    #[test]
    fn uses_override_for_supported_extension() {
        let path = PathBuf::from("image.cr2");
        assert_eq!(
            try_render_with_processor(&path, stub_jpeg),
            Some(vec![0xFF, 0xD8]),
        );
    }

    #[test]
    fn extension_check_is_case_insensitive() {
        let path = PathBuf::from("final.CR2");
        assert!(supports_path(&path));
        assert_eq!(
            try_render_with_processor(&path, stub_single_byte),
            Some(vec![0xFF])
        );
    }
}
