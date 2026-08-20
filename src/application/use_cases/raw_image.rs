//! Helper for rendering RAW/raster inputs via `jpgfromrawlib` so quick-view stays on the existing image path.

use jpgfromrawlib::{FindJpegType, SUPPORTED_EXTENSIONS, process_file_bytes};
use std::path::Path;
use std::sync::OnceLock;
use tokio::runtime::{Builder, Runtime};

const FIND_TYPE: FindJpegType = FindJpegType::Largest;

pub fn supports_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| ext.eq_ignore_ascii_case(supported))
        })
}

/// Attempts to produce JPEG bytes for supported RAW/raster files.
/// If the extension isn't listed in `SUPPORTED_EXTENSIONS`, returns `None`.
pub fn try_render_from_raw(path: &Path) -> Option<Vec<u8>> {
    if !supports_path(path) {
        return None;
    }
    runtime().block_on(process_file_bytes(path, FIND_TYPE)).ok()
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
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

    #[test]
    fn returns_none_for_unsupported_extension() {
        let path = PathBuf::from("photo.txt");
        assert!(try_render_from_raw(&path).is_none());
    }

    #[test]
    fn extension_check_is_case_insensitive() {
        assert!(supports_path(&PathBuf::from("image.cr2")));
        assert!(supports_path(&PathBuf::from("final.CR2")));
        assert!(!supports_path(&PathBuf::from("photo.txt")));
    }
}
