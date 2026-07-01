//! Recursive size use case - real (recursive, on-disk) size of a set of entries.
//!
//! Prefers shelling out to `du` (present on every Unix system, and dramatically faster
//! than a hand-rolled Rust walk: no per-file `Vec<FileEntry>` allocation, no HashSet
//! bookkeeping, just a tight, decades-optimized C loop) — the same "reuse a specialized
//! external tool" approach this app already takes for `fd`/`ripgrep`. Falls back to a
//! pure-Rust recursive walk via `FileSystemPort` when `du` isn't available (non-Unix,
//! or missing from `PATH`).

use crate::application::ports::FileSystemPort;
use crate::domain::FileEntry;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Sums the real (on-disk, recursive) size of `roots`: files contribute their own
/// disk usage, directories contribute the recursive usage of everything inside them.
/// Checked against `cancel` so a stopped computation exits early instead of running
/// to completion; the same `cancel` flag also lets a superseded `du` child process be
/// killed instead of waited out.
pub fn total_size(fs_port: &impl FileSystemPort, roots: &[FileEntry], cancel: &AtomicBool) -> u64 {
    #[cfg(unix)]
    if let Some(total) = total_size_via_du(roots, cancel) {
        return total;
    }
    total_size_fallback(fs_port, roots, cancel)
}

/// Pure-Rust recursive walk via `FileSystemPort::list_dir` (already returns per-entry
/// `size`, so only directory entries need a further walk). Used when `du` isn't
/// available, and directly by tests for exact byte-level assertions.
fn total_size_fallback(
    fs_port: &impl FileSystemPort,
    roots: &[FileEntry],
    cancel: &AtomicBool,
) -> u64 {
    let mut total = 0u64;
    for entry in roots {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        total += if entry.is_dir() {
            dir_size(fs_port, &entry.path, cancel)
        } else {
            entry.size
        };
    }
    total
}

fn dir_size(fs_port: &impl FileSystemPort, path: &Path, cancel: &AtomicBool) -> u64 {
    if cancel.load(Ordering::Relaxed) {
        return 0;
    }
    match fs_port.list_dir(path) {
        Ok(entries) => entries
            .iter()
            .map(|e| {
                if e.is_dir() {
                    dir_size(fs_port, &e.path, cancel)
                } else {
                    e.size
                }
            })
            .sum(),
        Err(_) => 0,
    }
}

/// Runs `du -sk -- <roots>` and sums the reported kilobyte totals. Returns `None` if
/// `du` isn't on `PATH`, fails to spawn, or the computation was cancelled (the caller
/// falls back to `total_size_fallback` in all of those cases, which itself respects
/// `cancel` and returns promptly if it's already set).
#[cfg(unix)]
fn total_size_via_du(roots: &[FileEntry], cancel: &AtomicBool) -> Option<u64> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    if roots.is_empty() {
        return Some(0);
    }

    let mut child = Command::new("du")
        .arg("-sk")
        .arg("--")
        .args(roots.iter().map(|e| &e.path))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Drain stdout on a separate thread while polling for exit/cancel below, so a
    // large `du` output can't fill the pipe buffer and deadlock the child.
    let mut stdout_pipe = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = stdout_pipe.read_to_string(&mut buf);
        buf
    });

    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return None;
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }

    let stdout = reader.join().ok()?;
    let total_kb: u64 = stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|kb| kb.parse::<u64>().ok())
        .sum();
    Some(total_kb * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EntryKind;
    use crate::infrastructure::StdFileSystem;
    use crate::test_support::create_temp_dir;
    use std::fs;

    fn entry(path: std::path::PathBuf, kind: EntryKind, size: u64) -> FileEntry {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        FileEntry::new(path, name, kind, size)
    }

    // total_size_fallback: exact, byte-level assertions (du reports block-rounded
    // disk usage, so these deliberately exercise the pure-Rust path directly).
    #[test]
    fn fallback_sums_file_only_roots() {
        let base = create_temp_dir("dir_size_files_only");
        let a = base.join("a.txt");
        let b = base.join("b.txt");
        fs::write(&a, "12345").unwrap();
        fs::write(&b, "1234567890").unwrap();
        let roots = vec![entry(a, EntryKind::File, 5), entry(b, EntryKind::File, 10)];
        let total = total_size_fallback(&StdFileSystem, &roots, &AtomicBool::new(false));
        assert_eq!(total, 15);
    }

    #[test]
    fn fallback_sums_recursive_contents_of_a_directory_root() {
        let base = create_temp_dir("dir_size_recursive");
        let dir = base.join("nested");
        fs::create_dir_all(dir.join("inner")).unwrap();
        fs::write(dir.join("top.txt"), "12345").unwrap(); // 5 bytes
        fs::write(dir.join("inner/deep.txt"), "1234567890").unwrap(); // 10 bytes

        let roots = vec![entry(dir, EntryKind::Dir, 0)];
        let total = total_size_fallback(&StdFileSystem, &roots, &AtomicBool::new(false));
        assert_eq!(total, 15);
    }

    #[test]
    fn fallback_mixes_files_and_directories_in_the_same_root_set() {
        let base = create_temp_dir("dir_size_mixed");
        let dir = base.join("sub");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("in_dir.txt"), "1234567890").unwrap(); // 10 bytes
        let file = base.join("top.txt");
        fs::write(&file, "12345").unwrap(); // 5 bytes

        let roots = vec![
            entry(file, EntryKind::File, 5),
            entry(dir, EntryKind::Dir, 0),
        ];
        let total = total_size_fallback(&StdFileSystem, &roots, &AtomicBool::new(false));
        assert_eq!(total, 15);
    }

    #[test]
    fn fallback_cancelled_walk_returns_without_hanging() {
        let base = create_temp_dir("dir_size_cancel");
        let dir = base.join("big");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "12345").unwrap();

        let cancel = AtomicBool::new(true);
        let roots = vec![entry(dir, EntryKind::Dir, 0)];
        // Must return promptly with a cancel flag already set, not hang or panic.
        let total = total_size_fallback(&StdFileSystem, &roots, &cancel);
        assert_eq!(total, 0);
    }

    // total_size: the public entrypoint, exercised through whichever path is
    // available on this machine (du on Unix, pure-Rust elsewhere).
    #[test]
    fn total_size_reports_at_least_the_apparent_size() {
        let base = create_temp_dir("dir_size_public_api");
        let dir = base.join("nested");
        fs::create_dir_all(dir.join("inner")).unwrap();
        fs::write(dir.join("top.txt"), "12345").unwrap();
        fs::write(dir.join("inner/deep.txt"), "1234567890").unwrap();

        let roots = vec![entry(dir, EntryKind::Dir, 0)];
        let total = total_size(&StdFileSystem, &roots, &AtomicBool::new(false));
        // Disk usage (du's block-rounded total, or the fallback's exact byte sum)
        // is always >= the apparent byte size of the actual file contents.
        assert!(
            total >= 15,
            "expected at least 15 bytes of real usage, got {total}"
        );
    }

    #[test]
    fn total_size_empty_roots_is_zero() {
        let total = total_size(&StdFileSystem, &[], &AtomicBool::new(false));
        assert_eq!(total, 0);
    }

    #[test]
    fn total_size_cancelled_up_front_returns_promptly() {
        let base = create_temp_dir("dir_size_public_cancel");
        let dir = base.join("big");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "12345").unwrap();

        let cancel = AtomicBool::new(true);
        let roots = vec![entry(dir, EntryKind::Dir, 0)];
        let total = total_size(&StdFileSystem, &roots, &cancel);
        assert_eq!(total, 0);
    }
}
