//! File transfer use case - copy/move with background progress and overwrite conflicts.
//!
//! `run_transfer` is the pure, synchronous engine (used directly in tests). `spawn_transfer`
//! wraps it on a background thread and turns conflict resolution into a blocking channel
//! round-trip so the caller (the UI thread) can show a prompt and reply.

use crate::application::ports::FileSystemPort;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferKind {
    Copy,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConflictResolution {
    Overwrite,
    Skip,
    OverwriteAll,
    SkipAll,
    Cancel,
}

#[derive(Debug)]
pub enum TransferEvent {
    Progress {
        current: PathBuf,
        done: usize,
        total: usize,
    },
    Conflict {
        path: PathBuf,
        reply: mpsc::Sender<ConflictResolution>,
    },
    Done(Outcome),
}

#[derive(Debug)]
pub struct Outcome {
    pub ok: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub cancelled: bool,
}

/// Starts `kind` transferring `sources` into `dest_dir` on a background thread.
/// `on_event` is invoked (from the background thread) for every progress/conflict/done
/// event; returns a cancellation flag the caller can set to stop the transfer between items.
pub fn spawn_transfer<FS: FileSystemPort + Send + 'static>(
    fs_port: FS,
    kind: TransferKind,
    sources: Vec<PathBuf>,
    dest_dir: PathBuf,
    on_event: impl Fn(TransferEvent) + Send + 'static,
) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_thread = Arc::clone(&cancel);
    thread::spawn(move || {
        let mut on_progress = |current: PathBuf, done: usize, total: usize| {
            on_event(TransferEvent::Progress {
                current,
                done,
                total,
            });
        };
        let mut resolve_conflict = |path: &Path| -> ConflictResolution {
            let (tx, rx) = mpsc::channel();
            on_event(TransferEvent::Conflict {
                path: path.to_path_buf(),
                reply: tx,
            });
            rx.recv().unwrap_or(ConflictResolution::Cancel)
        };
        let outcome = run_transfer(
            kind,
            &sources,
            &dest_dir,
            &fs_port,
            &mut on_progress,
            &mut resolve_conflict,
            &cancel_for_thread,
        );
        on_event(TransferEvent::Done(outcome));
    });
    cancel
}

enum ConflictPolicy {
    AskEachTime,
    AlwaysOverwrite,
    AlwaysSkip,
}

struct Ctx<'a, FS: FileSystemPort> {
    fs_port: &'a FS,
    on_progress: &'a mut dyn FnMut(PathBuf, usize, usize),
    resolve_conflict: &'a mut dyn FnMut(&Path) -> ConflictResolution,
    policy: ConflictPolicy,
    done: usize,
    total: usize,
    failed: Vec<(PathBuf, String)>,
}

enum ItemStatus {
    Cancelled,
    Done { fully_resolved: bool },
}

/// Synchronous transfer engine, driven by callbacks so it can run on a background
/// thread (via `spawn_transfer`) or be exercised directly in tests without threads.
pub fn run_transfer<FS: FileSystemPort>(
    kind: TransferKind,
    sources: &[PathBuf],
    dest_dir: &Path,
    fs_port: &FS,
    on_progress: &mut dyn FnMut(PathBuf, usize, usize),
    resolve_conflict: &mut dyn FnMut(&Path) -> ConflictResolution,
    cancel: &AtomicBool,
) -> Outcome {
    let total: usize = sources.iter().map(|s| count_items(s)).sum();
    let mut ctx = Ctx {
        fs_port,
        on_progress,
        resolve_conflict,
        policy: ConflictPolicy::AskEachTime,
        done: 0,
        total,
        failed: Vec::new(),
    };
    let mut cancelled = false;

    for src in sources {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        let Some(name) = src.file_name() else {
            ctx.failed
                .push((src.clone(), "Invalid source path".to_string()));
            continue;
        };
        let dest = dest_dir.join(name);
        match transfer_item(kind, src, &dest, &mut ctx, cancel) {
            ItemStatus::Cancelled => {
                cancelled = true;
                break;
            }
            ItemStatus::Done { .. } => {}
        }
    }

    let ok = ctx.done.saturating_sub(ctx.failed.len());
    Outcome {
        ok,
        failed: ctx.failed,
        cancelled,
    }
}

fn transfer_item<FS: FileSystemPort>(
    kind: TransferKind,
    src: &Path,
    dest: &Path,
    ctx: &mut Ctx<FS>,
    cancel: &AtomicBool,
) -> ItemStatus {
    if cancel.load(Ordering::Relaxed) {
        return ItemStatus::Cancelled;
    }
    if src.is_dir() {
        transfer_dir(kind, src, dest, ctx, cancel)
    } else {
        transfer_file(kind, src, dest, ctx)
    }
}

fn transfer_file<FS: FileSystemPort>(
    kind: TransferKind,
    src: &Path,
    dest: &Path,
    ctx: &mut Ctx<FS>,
) -> ItemStatus {
    if dest.exists() {
        match decide(dest, ctx) {
            ConflictResolution::Cancel => return ItemStatus::Cancelled,
            ConflictResolution::Skip => {
                bump(ctx, src, 1);
                return ItemStatus::Done {
                    fully_resolved: false,
                };
            }
            ConflictResolution::Overwrite
            | ConflictResolution::OverwriteAll
            | ConflictResolution::SkipAll => {}
        }
    }

    let result = match kind {
        TransferKind::Copy => ctx.fs_port.copy_file(src, dest).map_err(|e| e.to_string()),
        TransferKind::Move => move_file(src, dest, ctx.fs_port),
    };

    bump(ctx, src, 1);
    match result {
        Ok(()) => ItemStatus::Done {
            fully_resolved: true,
        },
        Err(e) => {
            ctx.failed.push((src.to_path_buf(), e));
            ItemStatus::Done {
                fully_resolved: false,
            }
        }
    }
}

fn transfer_dir<FS: FileSystemPort>(
    kind: TransferKind,
    src: &Path,
    dest: &Path,
    ctx: &mut Ctx<FS>,
    cancel: &AtomicBool,
) -> ItemStatus {
    if dest.exists() && dest.is_file() {
        match decide(dest, ctx) {
            ConflictResolution::Cancel => return ItemStatus::Cancelled,
            ConflictResolution::Skip => {
                bump(ctx, src, count_items(src));
                return ItemStatus::Done {
                    fully_resolved: false,
                };
            }
            ConflictResolution::Overwrite
            | ConflictResolution::OverwriteAll
            | ConflictResolution::SkipAll => {
                if let Err(e) = fs::remove_file(dest) {
                    ctx.failed.push((src.to_path_buf(), e.to_string()));
                    bump(ctx, src, count_items(src));
                    return ItemStatus::Done {
                        fully_resolved: false,
                    };
                }
            }
        }
    }
    if !dest.exists()
        && let Err(e) = ctx.fs_port.create_dir(dest)
    {
        ctx.failed.push((src.to_path_buf(), e.to_string()));
        bump(ctx, src, count_items(src));
        return ItemStatus::Done {
            fully_resolved: false,
        };
    }
    // dest now exists as a directory, either freshly created or a pre-existing merge target.
    bump(ctx, src, 1);

    let entries = match fs::read_dir(src) {
        Ok(rd) => rd,
        Err(e) => {
            ctx.failed.push((src.to_path_buf(), e.to_string()));
            return ItemStatus::Done {
                fully_resolved: false,
            };
        }
    };

    let mut all_resolved = true;
    for entry in entries.flatten() {
        let child_src = entry.path();
        let child_dest = dest.join(entry.file_name());
        match transfer_item(kind, &child_src, &child_dest, ctx, cancel) {
            ItemStatus::Cancelled => return ItemStatus::Cancelled,
            ItemStatus::Done { fully_resolved } => {
                if !fully_resolved {
                    all_resolved = false;
                }
            }
        }
    }

    if kind == TransferKind::Move && all_resolved {
        // Best-effort: a dir left non-empty by a skipped child simply won't remove.
        let _ = fs::remove_dir(src);
    }

    ItemStatus::Done {
        fully_resolved: all_resolved,
    }
}

fn decide<FS: FileSystemPort>(dest: &Path, ctx: &mut Ctx<FS>) -> ConflictResolution {
    match ctx.policy {
        ConflictPolicy::AlwaysOverwrite => return ConflictResolution::Overwrite,
        ConflictPolicy::AlwaysSkip => return ConflictResolution::Skip,
        ConflictPolicy::AskEachTime => {}
    }
    match (ctx.resolve_conflict)(dest) {
        ConflictResolution::OverwriteAll => {
            ctx.policy = ConflictPolicy::AlwaysOverwrite;
            ConflictResolution::Overwrite
        }
        ConflictResolution::SkipAll => {
            ctx.policy = ConflictPolicy::AlwaysSkip;
            ConflictResolution::Skip
        }
        other => other,
    }
}

fn bump<FS: FileSystemPort>(ctx: &mut Ctx<FS>, current: &Path, by: usize) {
    ctx.done += by;
    (ctx.on_progress)(current.to_path_buf(), ctx.done.min(ctx.total), ctx.total);
}

fn move_file(src: &Path, dest: &Path, fs_port: &impl FileSystemPort) -> Result<(), String> {
    match fs_port.rename(src, dest) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device(&e) => {
            fs_port.copy_file(src, dest).map_err(|e| e.to_string())?;
            fs::remove_file(src).map_err(|e| e.to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn is_cross_device(e: &std::io::Error) -> bool {
    // EXDEV = 18 on Linux/macOS
    e.raw_os_error() == Some(18) || e.kind() == std::io::ErrorKind::CrossesDevices
}

fn count_items(path: &Path) -> usize {
    if path.is_dir() {
        let mut n = 1;
        if let Ok(rd) = fs::read_dir(path) {
            for e in rd.flatten() {
                n += count_items(&e.path());
            }
        }
        n
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::StdFileSystem;
    use crate::test_support::create_temp_dir;
    use std::fs;

    fn no_conflicts(_: &Path) -> ConflictResolution {
        ConflictResolution::Overwrite
    }

    #[test]
    fn copies_single_file() {
        let base = create_temp_dir("transfer_copy_single");
        let src = base.join("a.txt");
        fs::write(&src, "hello").unwrap();
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();

        let mut progress_calls = 0;
        let mut on_progress = |_: PathBuf, _: usize, _: usize| progress_calls += 1;
        let mut resolve = no_conflicts;
        let outcome = run_transfer(
            TransferKind::Copy,
            std::slice::from_ref(&src),
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert_eq!(outcome.ok, 1);
        assert!(outcome.failed.is_empty());
        assert!(!outcome.cancelled);
        assert!(dest_dir.join("a.txt").exists());
        assert!(src.exists(), "copy must not remove the source");
        assert_eq!(progress_calls, 1);
    }

    #[test]
    fn moves_single_file() {
        let base = create_temp_dir("transfer_move_single");
        let src = base.join("a.txt");
        fs::write(&src, "hello").unwrap();
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();

        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let mut resolve = no_conflicts;
        let outcome = run_transfer(
            TransferKind::Move,
            std::slice::from_ref(&src),
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert_eq!(outcome.ok, 1);
        assert!(dest_dir.join("a.txt").exists());
        assert!(!src.exists(), "move must remove the source");
    }

    #[test]
    fn file_conflict_triggers_resolver_and_skip_leaves_dest_untouched() {
        let base = create_temp_dir("transfer_conflict_skip");
        let src = base.join("a.txt");
        fs::write(&src, "new").unwrap();
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(dest_dir.join("a.txt"), "old").unwrap();

        let mut asked = false;
        let mut resolve = |_: &Path| {
            asked = true;
            ConflictResolution::Skip
        };
        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let outcome = run_transfer(
            TransferKind::Copy,
            std::slice::from_ref(&src),
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert!(
            asked,
            "existing destination file must trigger a conflict prompt"
        );
        assert_eq!(fs::read_to_string(dest_dir.join("a.txt")).unwrap(), "old");
        assert!(src.exists(), "skipped source must remain in place");
        assert_eq!(outcome.ok, 1);
    }

    #[test]
    fn overwrite_all_resolves_remaining_conflicts_without_asking_again() {
        let base = create_temp_dir("transfer_overwrite_all");
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let mut sources = Vec::new();
        for name in ["a.txt", "b.txt", "c.txt"] {
            let src = base.join(name);
            fs::write(&src, "new").unwrap();
            fs::write(dest_dir.join(name), "old").unwrap();
            sources.push(src);
        }

        let mut ask_count = 0;
        let mut resolve = |_: &Path| {
            ask_count += 1;
            ConflictResolution::OverwriteAll
        };
        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let outcome = run_transfer(
            TransferKind::Copy,
            &sources,
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert_eq!(ask_count, 1, "only the first conflict should prompt");
        assert_eq!(outcome.ok, 3);
        for name in ["a.txt", "b.txt", "c.txt"] {
            assert_eq!(fs::read_to_string(dest_dir.join(name)).unwrap(), "new");
        }
    }

    #[test]
    fn directory_copy_merges_into_existing_directory() {
        let base = create_temp_dir("transfer_dir_merge");
        let src = base.join("src");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::write(src.join("only_in_src.txt"), "s").unwrap();
        fs::write(src.join("nested/child.txt"), "s-child").unwrap();

        let dest_dir = base.join("dest_parent");
        fs::create_dir_all(&dest_dir).unwrap();
        let existing_dest_dir = dest_dir.join("src");
        fs::create_dir_all(&existing_dest_dir).unwrap();
        fs::write(existing_dest_dir.join("only_in_dest.txt"), "d").unwrap();

        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let mut resolve = no_conflicts;
        let outcome = run_transfer(
            TransferKind::Copy,
            std::slice::from_ref(&src),
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert!(outcome.failed.is_empty());
        assert!(
            existing_dest_dir.join("only_in_dest.txt").exists(),
            "pre-existing file must survive merge"
        );
        assert!(existing_dest_dir.join("only_in_src.txt").exists());
        assert!(existing_dest_dir.join("nested/child.txt").exists());
    }

    #[test]
    fn directory_move_leaves_skipped_children_in_source() {
        let base = create_temp_dir("transfer_dir_move_partial");
        let src = base.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("keep.txt"), "new").unwrap();
        fs::write(src.join("also_move.txt"), "new2").unwrap();

        let dest_dir = base.join("dest_parent");
        fs::create_dir_all(&dest_dir).unwrap();
        let existing_dest_dir = dest_dir.join("src");
        fs::create_dir_all(&existing_dest_dir).unwrap();
        fs::write(existing_dest_dir.join("keep.txt"), "old").unwrap();

        let mut resolve = |_: &Path| ConflictResolution::Skip;
        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let outcome = run_transfer(
            TransferKind::Move,
            std::slice::from_ref(&src),
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert!(outcome.failed.is_empty());
        assert!(src.exists(), "source dir with a skipped child must remain");
        assert!(
            src.join("keep.txt").exists(),
            "skipped file must stay in source"
        );
        assert!(
            !src.join("also_move.txt").exists(),
            "non-conflicting file must still move"
        );
        assert!(existing_dest_dir.join("also_move.txt").exists());
        assert_eq!(
            fs::read_to_string(existing_dest_dir.join("keep.txt")).unwrap(),
            "old"
        );
    }

    #[test]
    fn one_failure_does_not_abort_the_batch() {
        let base = create_temp_dir("transfer_partial_failure");
        let missing = base.join("does_not_exist.txt");
        let present = base.join("present.txt");
        fs::write(&present, "hi").unwrap();
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();

        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let mut resolve = no_conflicts;
        let outcome = run_transfer(
            TransferKind::Copy,
            &[missing, present],
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert_eq!(outcome.failed.len(), 1);
        assert_eq!(outcome.ok, 1);
        assert!(dest_dir.join("present.txt").exists());
    }

    #[test]
    fn cancellation_stops_before_remaining_items() {
        let base = create_temp_dir("transfer_cancel");
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        let mut sources = Vec::new();
        for name in ["a.txt", "b.txt"] {
            let src = base.join(name);
            fs::write(&src, "x").unwrap();
            sources.push(src);
        }

        let cancel = AtomicBool::new(true);
        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let mut resolve = no_conflicts;
        let outcome = run_transfer(
            TransferKind::Copy,
            &sources,
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &cancel,
        );

        assert!(outcome.cancelled);
        assert!(!dest_dir.join("a.txt").exists());
    }

    #[test]
    fn directory_over_existing_file_is_a_conflict() {
        let base = create_temp_dir("transfer_dir_over_file");
        let src = base.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("child.txt"), "hi").unwrap();
        let dest_dir = base.join("dest_parent");
        fs::create_dir_all(&dest_dir).unwrap();
        // A plain file already sits where the directory wants to go.
        fs::write(dest_dir.join("src"), "blocking file").unwrap();

        let mut asked = false;
        let mut resolve = |_: &Path| {
            asked = true;
            ConflictResolution::Overwrite
        };
        let mut on_progress = |_: PathBuf, _: usize, _: usize| {};
        let outcome = run_transfer(
            TransferKind::Copy,
            std::slice::from_ref(&src),
            &dest_dir,
            &StdFileSystem,
            &mut on_progress,
            &mut resolve,
            &AtomicBool::new(false),
        );

        assert!(asked, "a directory landing on an existing file must prompt");
        assert!(outcome.failed.is_empty());
        assert!(dest_dir.join("src").is_dir());
        assert!(dest_dir.join("src/child.txt").exists());
    }

    #[test]
    fn spawn_transfer_runs_end_to_end_over_a_real_thread_and_conflict_channel() {
        let base = create_temp_dir("transfer_spawn_e2e");
        let src = base.join("a.txt");
        fs::write(&src, "new").unwrap();
        let dest_dir = base.join("dest");
        fs::create_dir_all(&dest_dir).unwrap();
        fs::write(dest_dir.join("a.txt"), "old").unwrap();

        let (tx, rx) = mpsc::channel::<TransferEvent>();
        let _cancel = spawn_transfer(
            StdFileSystem,
            TransferKind::Copy,
            vec![src.clone()],
            dest_dir.clone(),
            move |ev| {
                let _ = tx.send(ev);
            },
        );

        let mut done_event = None;
        for event in rx.iter() {
            if let TransferEvent::Conflict { reply, .. } = event {
                reply.send(ConflictResolution::Overwrite).unwrap();
                continue;
            }
            if let TransferEvent::Done(_) = event {
                done_event = Some(event);
                break;
            }
        }

        match done_event.expect("worker thread must send a Done event") {
            TransferEvent::Done(outcome) => {
                assert_eq!(outcome.ok, 1);
                assert!(outcome.failed.is_empty());
                assert!(!outcome.cancelled);
            }
            _ => unreachable!(),
        }
        assert_eq!(fs::read_to_string(dest_dir.join("a.txt")).unwrap(), "new");
    }
}
