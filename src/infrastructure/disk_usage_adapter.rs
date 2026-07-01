//! Disk usage adapter - free/total space of the volume containing a path.

use std::path::Path;

/// Returns `(free_bytes, total_bytes)` for the filesystem/volume containing `path`,
/// or `None` if it can't be determined (unsupported platform, or the syscall failed).
#[cfg(unix)]
pub fn disk_free_and_total(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return None;
    }
    let block_size = stat.f_frsize as u64;
    let free = stat.f_bavail as u64 * block_size;
    let total = stat.f_blocks as u64 * block_size;
    Some((free, total))
}

// ponytail: no Windows implementation (would need GetDiskFreeSpaceExW via a new
// windows-sys dependency, untestable in this environment); the footer simply omits
// the free/total figure on non-Unix targets.
#[cfg(not(unix))]
pub fn disk_free_and_total(_path: &Path) -> Option<(u64, u64)> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn reports_free_and_total_for_a_real_path() {
        let (free, total) = disk_free_and_total(Path::new("/")).expect("statvfs on / must succeed");
        assert!(total > 0);
        assert!(free <= total);
    }

    #[test]
    fn returns_none_for_a_path_with_interior_nul() {
        assert!(disk_free_and_total(Path::new("/tmp\0bad")).is_none());
    }
}
