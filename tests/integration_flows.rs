use fishez::application::ports::{ClipboardPort, OpenPort, SearchPort};
use fishez::application::use_cases::{file_ops, navigate, quick_view};
use fishez::application::{PanelMode, PanelState, QuickViewMode};
use fishez::domain::{EntryKind, FileEntry};
use fishez::infrastructure::favorites_adapter::save_favorites;
use fishez::infrastructure::{add_favorite, load_favorites};
use fishez::infrastructure::{FdSearchAdapter, RipGrepAdapter, StdFileSystem, SystemOpenAdapter};
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::process::Command;
use std::sync::Mutex;

static CWD_LOCK: Mutex<()> = Mutex::new(());

fn create_temp_dir(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    path.push(format!("{}_{}", prefix, nanos));
    fs::create_dir_all(&path).unwrap();
    path
}

fn command_available(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

fn open_command_available() -> bool {
    if cfg!(target_os = "windows") {
        Command::new("cmd").arg("/C").arg("echo").output().is_ok()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg("--version").output().is_ok()
    } else {
        Command::new("xdg-open").arg("--version").output().is_ok()
    }
}

#[test]
fn integration_search_fd_end_to_end() {
    if !command_available("fd") {
        return;
    }

    let base = create_temp_dir("search_fd");
    let dir_path = base.join("alpha_dir");
    let file_path = base.join("alpha.txt");
    fs::create_dir_all(&dir_path).unwrap();
    fs::write(dir_path.join("nested.txt"), "alpha nested").unwrap();
    fs::write(&file_path, "alpha").unwrap();

    let results = FdSearchAdapter::new().find("", &base);
    assert!(results.contains(&"alpha.txt".to_string()));
    assert!(results.contains(&format!("alpha_dir{}nested.txt", MAIN_SEPARATOR)));
}

#[test]
fn integration_search_rg_end_to_end() {
    if !command_available("rg") {
        return;
    }

    let base = create_temp_dir("search_rg");
    let file_path = base.join("notes.txt");
    fs::write(&file_path, "needle in a haystack").unwrap();

    let results = RipGrepAdapter::new().find("needle", &base);
    assert!(results.contains(&"notes.txt".to_string()));
}

#[test]
fn integration_navigation_refresh_entries() {
    let base = create_temp_dir("navigate_refresh");
    let dir_path = base.join("docs");
    let file_path = base.join("file.txt");
    fs::create_dir_all(&dir_path).unwrap();
    fs::write(&file_path, "hello").unwrap();

    let fs_adapter = StdFileSystem::new();
    let mut panel = PanelState::new();
    panel.current_path = base.clone();
    panel.mode = PanelMode::Normal;
    navigate::refresh_entries(&fs_adapter, &mut panel);
    assert!(panel.entries.iter().any(|e| e.name == ".."));
    assert!(panel
        .entries
        .iter()
        .any(|e| e.name == "file.txt" && e.kind == EntryKind::File));
    assert!(panel
        .entries
        .iter()
        .any(|e| e.name == format!("docs{}", MAIN_SEPARATOR)));

    panel.filter_string = "file".to_string();
    navigate::refresh_entries(&fs_adapter, &mut panel);
    assert!(panel.entries.iter().any(|e| e.name == "file.txt"));
    assert!(!panel
        .entries
        .iter()
        .any(|e| e.name == format!("docs{}", MAIN_SEPARATOR)));
}

#[test]
fn integration_quick_view_text() {
    let base = create_temp_dir("quick_view_text");
    let file_path = base.join("readme.txt");
    fs::write(&file_path, "hello world").unwrap();

    let entry = FileEntry::new(
        file_path.clone(),
        "readme.txt".to_string(),
        EntryKind::File,
        0,
    );
    let mut panel = PanelState::new();
    panel.entries.push(entry);
    panel.cursor = 0;

    quick_view::open(&mut panel, 40);
    assert!(matches!(
        panel.mode,
        PanelMode::QuickView(QuickViewMode::Text { .. })
    ));
}

#[test]
fn integration_quick_view_directory() {
    let base = create_temp_dir("quick_view_dir");
    fs::write(base.join("file.txt"), "data").unwrap();

    let entry = FileEntry::new(base.clone(), "docs".to_string(), EntryKind::Dir, 0);
    let mut panel = PanelState::new();
    panel.entries.push(entry);
    panel.cursor = 0;

    quick_view::open(&mut panel, 40);
    assert!(matches!(
        panel.mode,
        PanelMode::QuickView(QuickViewMode::Directory { .. })
    ));
}

#[test]
fn integration_favorites_persistence() {
    let _guard = CWD_LOCK.lock().unwrap();
    let base = create_temp_dir("favorites_integration");
    let original = std::env::current_dir().unwrap();
    std::env::set_current_dir(&base).unwrap();

    let mut items = vec![];
    assert!(add_favorite(&mut items, Path::new("/tmp/integration")));
    assert!(!add_favorite(&mut items, Path::new("/tmp/integration")));
    let loaded = load_favorites();
    assert_eq!(loaded, vec!["/tmp/integration".to_string()]);

    save_favorites(&["/tmp/a".to_string(), "/tmp/b".to_string()]);
    let loaded = load_favorites();
    assert_eq!(loaded, vec!["/tmp/a".to_string(), "/tmp/b".to_string()]);

    std::env::set_current_dir(original).unwrap();
}

#[test]
fn integration_copy_to_clipboard() {
    struct MockClipboard {
        content: Option<String>,
    }

    impl ClipboardPort for MockClipboard {
        fn copy(&mut self, text: &str) -> Result<(), String> {
            self.content = Some(text.to_string());
            Ok(())
        }
    }

    let mut clipboard = MockClipboard { content: None };
    let mut panel = PanelState::new();
    panel.entries.push(FileEntry::new(
        PathBuf::from("/tmp/file.txt"),
        "file.txt".to_string(),
        EntryKind::File,
        0,
    ));
    panel.cursor = 0;

    let result = file_ops::copy_to_clipboard(&mut clipboard, &mut panel, false);
    assert!(result.is_ok());
    assert_eq!(clipboard.content, Some("file.txt".to_string()));
}

#[test]
fn integration_refresh_entries_filters_case_insensitive() {
    let base = create_temp_dir("navigate_filter_case");
    fs::write(base.join("Readme.MD"), "text").unwrap();
    let fs_adapter = StdFileSystem::new();
    let mut panel = PanelState::new();
    panel.current_path = base;
    panel.mode = PanelMode::Filter;
    panel.filter_string = "readme".to_string();
    navigate::refresh_entries(&fs_adapter, &mut panel);
    assert!(panel.entries.iter().any(|e| e.name == "Readme.MD"));
}

#[test]
fn integration_open_adapter_spawns() {
    if std::env::var("FISHEZ_RUN_OPEN_TESTS").is_err() {
        return;
    }
    if !open_command_available() {
        return;
    }

    let base = create_temp_dir("open_adapter");
    let file_path = base.join("open_me.txt");
    fs::write(&file_path, "open test").unwrap();

    let adapter = SystemOpenAdapter::new();
    adapter.open(&file_path);
}

#[test]
fn integration_trash_delete_selected() {
    if std::env::var("FISHEZ_RUN_TRASH_TESTS").is_err() {
        return;
    }

    let base = create_temp_dir("trash_delete");
    let file_path = base.join("trash_me.txt");
    fs::write(&file_path, "delete test").unwrap();

    let fs_adapter = StdFileSystem::new();
    let mut panel = PanelState::new();
    panel.current_path = base.clone();
    panel.entries = vec![FileEntry::new(
        file_path.clone(),
        "trash_me.txt".to_string(),
        EntryKind::File,
        0,
    )];
    panel.cursor = 0;
    let paths: Vec<&Path> = vec![file_path.as_path()];
    let result = file_ops::delete_selected(&fs_adapter, &mut panel, &paths);
    assert!(result.is_ok());
    assert!(!file_path.exists());
}
