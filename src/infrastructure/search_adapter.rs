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
        return result
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
            .collect();
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
        let mut results = Vec::new();
        let result_str = String::from_utf8_lossy(&output.stdout);
        for line in result_str.lines() {
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
        return results;
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
            return result
                .split('\0')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
        }
        Vec::new()
    }
}
