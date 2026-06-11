use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    Labels, collect_profile_ids, label_for_id, normalize_managed_files, profile_path_for_id,
};
use crate::{
    PROFILE_ERR_INDEX_INVALID_JSON, PROFILE_ERR_READ_INDEX, PROFILE_ERR_SERIALIZE_INDEX,
    PROFILE_ERR_WRITE_INDEX, Paths, Tokens, extract_email_and_plan, extract_profile_identity,
    format_warning, is_api_key_profile, is_profile_ready, lock_usage, normalize_error, read_tokens,
    token_account_id, use_color_stderr, write_atomic,
};

pub(crate) const PROFILES_INDEX_VERSION: u8 = 3;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ProfilesIndex {
    #[serde(default = "profiles_index_version")]
    pub(crate) version: u8,
    #[serde(default)]
    pub(crate) profiles: BTreeMap<String, ProfileIndexEntry>,
}

impl Default for ProfilesIndex {
    fn default() -> Self {
        Self {
            version: PROFILES_INDEX_VERSION,
            profiles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ProfileIndexEntry {
    #[serde(default)]
    pub(crate) account_id: Option<String>,
    #[serde(default)]
    pub(crate) email: Option<String>,
    #[serde(default)]
    pub(crate) plan: Option<String>,
    #[serde(default)]
    pub(crate) label: Option<String>,
    #[serde(default)]
    pub(crate) is_api_key: bool,
    #[serde(default)]
    pub(crate) principal_id: Option<String>,
    #[serde(default)]
    pub(crate) workspace_or_org_id: Option<String>,
    #[serde(default)]
    pub(crate) plan_type_key: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) managed_files: Vec<String>,
}

fn profiles_index_version() -> u8 {
    PROFILES_INDEX_VERSION
}

fn has_legacy_schema(contents: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(contents)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .map(|obj| {
            obj.contains_key("last_used")
                || obj.contains_key("active_profile_id")
                || obj.contains_key("update_cache")
                || obj.contains_key("default_profile_id")
        })
        .unwrap_or(false)
}

pub(crate) fn read_profiles_index(paths: &Paths) -> Result<ProfilesIndex, String> {
    if !paths.profiles_index.exists() {
        return Ok(ProfilesIndex::default());
    }
    let contents = fs::read_to_string(&paths.profiles_index)
        .map_err(|err| crate::msg2(PROFILE_ERR_READ_INDEX, paths.profiles_index.display(), err))?;
    let had_legacy_schema = has_legacy_schema(&contents);
    let mut index: ProfilesIndex = serde_json::from_str(&contents).map_err(|_| {
        crate::msg1(
            PROFILE_ERR_INDEX_INVALID_JSON,
            paths.profiles_index.display(),
        )
    })?;
    if index.version < PROFILES_INDEX_VERSION {
        index.version = PROFILES_INDEX_VERSION;
    }
    if had_legacy_schema {
        let _ = write_profiles_index(paths, &index);
    }
    Ok(index)
}

pub(crate) fn read_profiles_index_relaxed(paths: &Paths) -> ProfilesIndex {
    match read_profiles_index(paths) {
        Ok(index) => index,
        Err(err) => {
            let normalized = normalize_error(&err);
            let warning = format_warning(&normalized, use_color_stderr());
            eprintln!("{warning}");
            ProfilesIndex::default()
        }
    }
}

pub(crate) fn write_profiles_index(paths: &Paths, index: &ProfilesIndex) -> Result<(), String> {
    let json = serde_json::to_string_pretty(index)
        .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?;
    crate::common::write_atomic_private(&paths.profiles_index, format!("{json}\n").as_bytes())
        .map_err(|err| crate::msg1(PROFILE_ERR_WRITE_INDEX, err))
}

pub(crate) fn repair_profiles_metadata(paths: &Paths) -> Result<Vec<String>, String> {
    let _lock = lock_usage(paths)?;

    let had_index = paths.profiles_index.exists();
    let mut repairs = Vec::new();
    let mut should_write = false;
    let mut normalized_index = false;
    let mut index = if !had_index {
        should_write = true;
        repairs.push("Initialized profiles index".to_string());
        ProfilesIndex::default()
    } else {
        let contents = fs::read_to_string(&paths.profiles_index).map_err(|err| {
            crate::msg2(PROFILE_ERR_READ_INDEX, paths.profiles_index.display(), err)
        })?;
        let had_legacy_schema = has_legacy_schema(&contents);
        match serde_json::from_str::<ProfilesIndex>(&contents) {
            Ok(mut index) => {
                if index.version < PROFILES_INDEX_VERSION {
                    index.version = PROFILES_INDEX_VERSION;
                    normalized_index = true;
                }
                if had_legacy_schema {
                    normalized_index = true;
                }
                if normalized_index {
                    should_write = true;
                    repairs.push("Normalized profiles index format".to_string());
                }
                index
            }
            Err(_) => {
                should_write = true;
                let backup_path = next_profiles_index_backup_path(&paths.profiles_index);
                write_atomic(&backup_path, contents.as_bytes())?;
                repairs.push(format!(
                    "Backed up invalid profiles index to {}",
                    backup_path.display()
                ));
                repairs.push("Rebuilt invalid profiles index".to_string());
                ProfilesIndex::default()
            }
        }
    };

    let ids = collect_profile_ids(&paths.profiles)?;
    let before_entries = index.profiles.len();

    prune_profiles_index(&mut index, &paths.profiles)?;
    let pruned = before_entries.saturating_sub(index.profiles.len());
    if pruned > 0 {
        should_write = true;
        repairs.push(format!(
            "Pruned {pruned} stale profile index {}",
            if pruned == 1 { "entry" } else { "entries" }
        ));
    }

    let mut indexed = 0usize;
    for id in ids {
        if index.profiles.contains_key(&id) {
            continue;
        }
        let path = profile_path_for_id(&paths.profiles, &id);
        match read_tokens(&path) {
            Ok(tokens) if is_profile_ready(&tokens) => {}
            _ => continue,
        }
        index.profiles.insert(id, ProfileIndexEntry::default());
        indexed += 1;
    }
    if indexed > 0 {
        should_write = true;
        repairs.push(format!(
            "Indexed {indexed} saved {}",
            if indexed == 1 { "profile" } else { "profiles" }
        ));
    }

    if should_write {
        write_profiles_index(paths, &index)?;
    }
    Ok(repairs)
}

fn next_profiles_index_backup_path(path: &Path) -> PathBuf {
    let base = path.with_extension("json.bak");
    if !base.exists() {
        return base;
    }
    let mut idx = 1usize;
    loop {
        let candidate = path.with_extension(format!("json.bak.{idx}"));
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
    }
}

pub(crate) fn prune_profiles_index(
    index: &mut ProfilesIndex,
    profiles_dir: &Path,
) -> Result<(), String> {
    let ids = collect_profile_ids(profiles_dir)?;
    index.profiles.retain(|id, _| ids.contains(id));
    Ok(())
}

pub(crate) fn sync_profiles_index(index: &mut ProfilesIndex, labels: &Labels) {
    for (id, entry) in index.profiles.iter_mut() {
        entry.label = label_for_id(labels, id);
    }
}

pub(crate) fn labels_from_index(index: &ProfilesIndex) -> Labels {
    let mut labels = Labels::new();
    for (id, entry) in &index.profiles {
        let Some(label) = entry.label.as_deref() else {
            continue;
        };
        let trimmed = label.trim();
        if trimmed.is_empty() || labels.contains_key(trimmed) {
            continue;
        }
        labels.insert(trimmed.to_string(), id.clone());
    }
    labels
}

pub(crate) fn update_profiles_index_entry(
    index: &mut ProfilesIndex,
    id: &str,
    tokens: Option<&Tokens>,
    label: Option<String>,
    managed_files: Option<Vec<String>>,
) {
    let entry = index.profiles.entry(id.to_string()).or_default();
    if let Some(tokens) = tokens {
        let (email, plan) = extract_email_and_plan(tokens);
        entry.email = email;
        entry.plan = plan;
        entry.account_id = token_account_id(tokens).map(str::to_string);
        entry.is_api_key = is_api_key_profile(tokens);
        if let Some(identity) = extract_profile_identity(tokens) {
            entry.principal_id = Some(identity.principal_id);
            entry.workspace_or_org_id = Some(identity.workspace_or_org_id);
            entry.plan_type_key = Some(identity.plan_type);
        }
    }
    if let Some(label) = label {
        entry.label = Some(label);
    }
    if let Some(managed_files) = managed_files {
        entry.managed_files = normalize_managed_files(managed_files);
    }
}
