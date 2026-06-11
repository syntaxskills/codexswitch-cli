use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{AUTH_FILE_NAME, CONFIG_FILE_NAME};
use crate::PROFILE_ERR_READ_PROFILES_DIR;

pub(crate) fn profile_files(profiles_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !profiles_dir.exists() {
        return Ok(files);
    }
    let entries = fs::read_dir(profiles_dir)
        .map_err(|err| crate::msg1(PROFILE_ERR_READ_PROFILES_DIR, err))?;
    for entry in entries {
        let entry = entry.map_err(|err| crate::msg1(PROFILE_ERR_READ_PROFILES_DIR, err))?;
        let path = entry.path();
        if path.is_dir() {
            let auth_path = path.join(AUTH_FILE_NAME);
            if auth_path.is_file() {
                files.push(auth_path);
            }
        }
    }
    Ok(files)
}

pub(crate) fn profile_id_from_path(path: &Path) -> Option<String> {
    if path.file_name().and_then(|value| value.to_str()) == Some(AUTH_FILE_NAME) {
        return path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|value| value.to_str())
            .filter(|stem| !stem.is_empty())
            .map(|stem| stem.to_string());
    }
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|stem| !stem.is_empty())
        .map(|stem| stem.to_string())
}

pub(crate) fn profile_dir_for_id(profiles_dir: &Path, id: &str) -> PathBuf {
    profiles_dir.join(id)
}

pub(crate) fn profile_path_for_id(profiles_dir: &Path, id: &str) -> PathBuf {
    profile_dir_for_id(profiles_dir, id).join(AUTH_FILE_NAME)
}

pub(crate) fn profile_config_path_for_id(profiles_dir: &Path, id: &str) -> PathBuf {
    profile_dir_for_id(profiles_dir, id).join(CONFIG_FILE_NAME)
}

pub(super) fn ensure_profile_dir(profiles_dir: &Path, id: &str) -> Result<(), String> {
    let dir = profile_dir_for_id(profiles_dir, id);
    fs::create_dir_all(&dir).map_err(|err| {
        format!(
            "Error: Cannot create profile directory {}: {err}",
            dir.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|err| {
            format!(
                "Error: Cannot set permissions on profile directory {}: {err}",
                dir.display()
            )
        })?;
    }
    Ok(())
}

pub(super) fn remove_profile_dir_if_empty(profiles_dir: &Path, id: &str) -> Result<(), String> {
    let dir = profile_dir_for_id(profiles_dir, id);
    match fs::remove_dir(&dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

pub(crate) fn collect_profile_ids(profiles_dir: &Path) -> Result<HashSet<String>, String> {
    let mut ids = HashSet::new();
    for path in profile_files(profiles_dir)? {
        if let Some(stem) = profile_id_from_path(&path) {
            ids.insert(stem);
        }
    }
    Ok(ids)
}
