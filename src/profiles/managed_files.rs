use colored::Colorize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::{AUTH_FILE_NAME, CONFIG_FILE_NAME, ProfileIndexEntry, profile_config_path_for_id};
use crate::{Paths, style_text};

pub(super) fn managed_files_for_save(include_config: bool) -> Vec<String> {
    let mut files = vec![AUTH_FILE_NAME.to_string()];
    if include_config {
        files.push(CONFIG_FILE_NAME.to_string());
    }
    files
}

pub(super) fn active_managed_files(paths: &Paths) -> Vec<String> {
    managed_files_for_save(paths.config.is_file())
}

pub(super) fn managed_files_for_profile(
    profiles_dir: &Path,
    id: &str,
    index_entry: Option<&ProfileIndexEntry>,
) -> Vec<String> {
    let mut files = index_entry
        .map(|entry| entry.managed_files.clone())
        .unwrap_or_default();
    if files.is_empty() {
        files.push(AUTH_FILE_NAME.to_string());
    }
    if profile_config_path_for_id(profiles_dir, id).is_file()
        && !files.iter().any(|file| file == CONFIG_FILE_NAME)
    {
        files.push(CONFIG_FILE_NAME.to_string());
    }
    normalize_managed_files(files)
}

pub(super) fn normalize_managed_files(files: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = vec![AUTH_FILE_NAME.to_string()];
    seen.insert(AUTH_FILE_NAME.to_string());
    let mut has_config = false;
    let mut custom = Vec::new();

    for file in files {
        let file = file.trim();
        if file.is_empty() || file == AUTH_FILE_NAME {
            continue;
        }
        if file == CONFIG_FILE_NAME {
            has_config = true;
            continue;
        }
        if seen.insert(file.to_string()) {
            custom.push(file.to_string());
        }
    }

    if has_config {
        out.push(CONFIG_FILE_NAME.to_string());
    }
    out.extend(custom);
    out
}

pub(super) fn managed_files_contains_config(files: &[String]) -> bool {
    files.iter().any(|file| file == CONFIG_FILE_NAME)
}

pub(super) fn format_managed_files(files: &[String]) -> String {
    files.join(" + ")
}

pub(super) fn format_managed_files_suffix(files: &[String], use_color: bool) -> String {
    style_text(
        &format!(" [files: {}]", format_managed_files(files)),
        use_color,
        |text| text.dimmed(),
    )
}

pub(super) fn format_managed_files_line(files: &[String], use_color: bool) -> String {
    style_text(
        &format!("[files: {}]", format_managed_files(files)),
        use_color,
        |text| text.dimmed(),
    )
}

pub(super) fn remove_profile_config_if_present(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|err| {
        format!(
            "Error: Could not remove saved config {}: {err}",
            path.display()
        )
    })
}
