//! Search adapter - implements SearchPort using fd/rg commands.

use crate::application::ports::SearchPort;
use std::path::{MAIN_SEPARATOR, Path};
use std::process::Command;

/// Search adapter using fd command.
pub struct FdSearchAdapter;

impl Default for FdSearchAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FdSearchAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl SearchPort for FdSearchAdapter {
    fn find(&self, query: &str, path: &Path) -> Vec<String> {
        if cfg!(target_os = "windows") {
            find_windows(query, path)
        } else {
            find_unix(query, path)
        }
    }
}

fn find_unix(filter: &str, dir: &Path) -> Vec<String> {
    let output = Command::new("fd").arg(filter).current_dir(dir).output();
    if let Ok(output) = output
        && output.status.success()
    {
        let result = String::from_utf8_lossy(&output.stdout);
        return parse_fd_output(&result, dir);
    }
    Vec::new()
}

fn find_windows(filter: &str, dir: &Path) -> Vec<String> {
    let output = Command::new("cmd")
        .args(["/C", "dir", "/s", "/b", &format!("*{}*", filter)])
        .current_dir(dir)
        .output();

    if let Ok(output) = output
        && output.status.success()
    {
        let result_str = String::from_utf8_lossy(&output.stdout);
        return parse_windows_output(&result_str, dir);
    }
    Vec::new()
}

/// RipGrep search adapter.
pub struct RipGrepAdapter;

impl Default for RipGrepAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RipGrepAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl SearchPort for RipGrepAdapter {
    fn find(&self, query: &str, path: &Path) -> Vec<String> {
        let output = Command::new("rg")
            .args(["--files-with-matches", "-0", query])
            .current_dir(path)
            .output();

        if let Ok(output) = output
            && output.status.success()
        {
            let result = String::from_utf8_lossy(&output.stdout);
            return parse_rg_output(&result);
        }
        Vec::new()
    }
}

fn parse_fd_output(output: &str, dir: &Path) -> Vec<String> {
    output
        .lines()
        .map(|s| s.to_string())
        .map(|rel| {
            let abs = dir.join(&rel);
            if abs.is_dir() {
                format!("{}{}", rel, MAIN_SEPARATOR)
            } else {
                rel
            }
        })
        .collect()
}

fn parse_windows_output(output: &str, dir: &Path) -> Vec<String> {
    let mut results = Vec::new();
    for line in output.lines() {
        let abs_path = Path::new(line);
        let rel = abs_path
            .strip_prefix(dir)
            .unwrap_or(abs_path)
            .to_string_lossy()
            .to_string();
        if abs_path.is_dir() {
            results.push(format!("{}{}", rel, MAIN_SEPARATOR));
        } else {
            results.push(rel);
        }
    }
    results
}

fn parse_rg_output(output: &str) -> Vec<String> {
    output
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

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

    #[test]
    fn test_parse_fd_output_marks_directories() {
        let base = create_temp_dir("fd_parse");
        let dir_path = base.join("docs");
        let file_path = base.join("file.txt");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(&file_path, "data").unwrap();

        let output = "docs\nfile.txt\n";
        let results = parse_fd_output(output, &base);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&format!("docs{}", MAIN_SEPARATOR)));
        assert!(results.contains(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_windows_output_strips_base() {
        let base = create_temp_dir("win_parse");
        let dir_path = base.join("docs");
        let file_path = base.join("file.txt");
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(&file_path, "data").unwrap();

        let output = format!("{}\n{}\n", dir_path.display(), file_path.display());
        let results = parse_windows_output(&output, &base);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&format!("docs{}", MAIN_SEPARATOR)));
        assert!(results.contains(&"file.txt".to_string()));
    }

    #[test]
    fn test_parse_rg_output_splits_null_delimited() {
        let output = "file1\0file2\0";
        let results = parse_rg_output(output);
        assert_eq!(results, vec!["file1".to_string(), "file2".to_string()]);
    }
}
