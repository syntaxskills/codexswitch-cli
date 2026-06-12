use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use super::{
    Labels, ProfilesIndex, cached_profile_ids, collect_profile_ids, ensure_profile_dir,
    label_for_id, labels_from_index, managed_files_contains_config, managed_files_for_profile,
    pick_primary, profile_config_path_for_id, profile_files, profile_id_from_path,
    profile_path_for_id, prune_labels, prune_profiles_index, read_profiles_index,
    read_profiles_index_relaxed, resolve_sync_id, sync_profiles_index, update_profiles_index_entry,
    write_config_snapshot, write_profiles_index,
};
use crate::{
    PROFILE_ERR_SYNC_CURRENT, PROFILE_UNSAVED_NO_MATCH, Paths, Tokens, UsageLock, copy_atomic,
    extract_profile_identity, lock_usage, normalize_error, read_tokens, read_tokens_opt,
};

pub fn load_profile_tokens_map(
    paths: &Paths,
) -> Result<BTreeMap<String, Result<Tokens, String>>, String> {
    let mut map = BTreeMap::new();
    for path in profile_files(&paths.profiles)? {
        let Some(stem) = profile_id_from_path(&path) else {
            continue;
        };
        match read_tokens(&path) {
            Ok(tokens) => {
                map.insert(stem, Ok(tokens));
            }
            Err(err) => {
                map.insert(stem, Err(normalize_error(&err)));
            }
        }
    }
    Ok(map)
}

pub(crate) struct Snapshot {
    pub(crate) labels: Labels,
    pub(crate) tokens: BTreeMap<String, Result<Tokens, String>>,
    pub(crate) index: ProfilesIndex,
}

pub(crate) fn sync_current(paths: &Paths, index: &mut ProfilesIndex) -> Result<(), String> {
    let Some(tokens) = read_tokens_opt(&paths.auth) else {
        return Ok(());
    };
    let id = match resolve_sync_id(paths, index, &tokens)? {
        Some(id) => id,
        None => return Ok(()),
    };
    let target = profile_path_for_id(&paths.profiles, &id);
    let managed_files = managed_files_for_profile(&paths.profiles, &id, index.profiles.get(&id));
    ensure_profile_dir(&paths.profiles, &id)?;
    sync_profile(paths, &target)?;
    if managed_files_contains_config(&managed_files) {
        sync_profile_config(paths, &id)?;
    }
    let label = label_for_id(&labels_from_index(index), &id);
    update_profiles_index_entry(index, &id, Some(&tokens), label, Some(managed_files));
    Ok(())
}

pub(crate) fn sync_profile(paths: &Paths, target: &Path) -> Result<(), String> {
    sync_file(&paths.auth, target)
}

fn sync_profile_config(paths: &Paths, id: &str) -> Result<(), String> {
    if !paths.config.is_file() {
        return Ok(());
    }
    ensure_profile_dir(&paths.profiles, id)?;
    let target = profile_config_path_for_id(&paths.profiles, id);
    write_config_snapshot(paths, &paths.config, &target)
        .map_err(|err| crate::msg1(PROFILE_ERR_SYNC_CURRENT, err))
}

fn sync_file(source: &Path, target: &Path) -> Result<(), String> {
    copy_atomic(source, target).map_err(|err| crate::msg1(PROFILE_ERR_SYNC_CURRENT, err))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(target, fs::Permissions::from_mode(0o600))
            .map_err(|err| crate::msg1(PROFILE_ERR_SYNC_CURRENT, err))?;
    }
    Ok(())
}

pub(crate) fn load_snapshot(paths: &Paths, strict_labels: bool) -> Result<Snapshot, String> {
    let _lock = lock_usage(paths)?;
    let tokens = load_profile_tokens_map(paths)?;
    let ids: HashSet<String> = tokens.keys().cloned().collect();
    let mut index = if strict_labels {
        read_profiles_index(paths)?
    } else {
        read_profiles_index_relaxed(paths)
    };
    let _ = prune_profiles_index(&mut index, &paths.profiles);
    for id in &ids {
        index.profiles.entry(id.clone()).or_default();
    }
    let labels = labels_from_index(&index);

    Ok(Snapshot {
        labels,
        tokens,
        index,
    })
}

pub(crate) fn unsaved_reason(
    paths: &Paths,
    tokens_map: &BTreeMap<String, Result<Tokens, String>>,
) -> Option<String> {
    let tokens = read_tokens_opt(&paths.auth)?;
    let identity = extract_profile_identity(&tokens)?;
    let candidates = cached_profile_ids(tokens_map, &identity);
    if candidates.is_empty() {
        return Some(PROFILE_UNSAVED_NO_MATCH.to_string());
    }
    None
}

pub(crate) fn current_saved_id(
    paths: &Paths,
    tokens_map: &BTreeMap<String, Result<Tokens, String>>,
) -> Option<String> {
    let tokens = read_tokens_opt(&paths.auth)?;
    let identity = extract_profile_identity(&tokens)?;
    let candidates = cached_profile_ids(tokens_map, &identity);
    pick_primary(&candidates)
}

pub(crate) struct ProfileStore {
    _lock: UsageLock,
    pub(crate) labels: Labels,
    pub(crate) profiles_index: ProfilesIndex,
}

impl ProfileStore {
    pub(crate) fn load(paths: &Paths) -> Result<Self, String> {
        let lock = lock_usage(paths)?;
        let mut profiles_index = read_profiles_index_relaxed(paths);
        let _ = prune_profiles_index(&mut profiles_index, &paths.profiles);
        let ids = collect_profile_ids(&paths.profiles)?;
        for id in &ids {
            profiles_index.profiles.entry(id.clone()).or_default();
        }
        let labels = labels_from_index(&profiles_index);
        Ok(Self {
            _lock: lock,
            labels,
            profiles_index,
        })
    }

    pub(crate) fn save(&mut self, paths: &Paths) -> Result<(), String> {
        prune_labels(&mut self.labels, &paths.profiles);
        prune_profiles_index(&mut self.profiles_index, &paths.profiles)?;
        sync_profiles_index(&mut self.profiles_index, &self.labels);
        write_profiles_index(paths, &self.profiles_index)?;
        Ok(())
    }
}
