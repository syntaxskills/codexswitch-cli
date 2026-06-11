use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{
    CONFIG_FILE_NAME, ProfileStore, assign_label, collect_profile_ids, ensure_profile_dir,
    format_action, format_list_hint, label_for_id, managed_files_contains_config,
    managed_files_for_profile, normalize_managed_files, print_output_block,
    profile_config_path_for_id, profile_path_for_id, remove_profile_dir_if_empty,
    resolve_label_target_id, update_profiles_index_entry, use_color_stderr, use_color_stdout,
};
use crate::json_response::CommandResultJson;
use crate::{
    AuthFile, PROFILE_ERR_ID_NO_MATCH, PROFILE_ERR_READ_PROFILES_DIR, Paths, is_profile_ready,
    tokens_from_api_key,
};

#[derive(Serialize, Deserialize)]
struct ExportBundle {
    version: u8,
    profiles: Vec<ExportedProfile>,
}

#[derive(Serialize, Deserialize)]
struct ExportedProfile {
    id: String,
    label: Option<String>,
    contents: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    managed_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    config_toml: Option<String>,
}

struct PreparedImportProfile {
    id: String,
    label: Option<String>,
    contents: Vec<u8>,
    managed_files: Vec<String>,
    config_toml: Option<Vec<u8>>,
    tokens: crate::Tokens,
}

pub(crate) fn export_profiles(
    paths: &Paths,
    label: Option<String>,
    ids: Vec<String>,
    output: PathBuf,
    json: bool,
) -> Result<(), String> {
    if output.exists() {
        return Err(format!(
            "Error: Export file already exists: {}",
            output.display()
        ));
    }

    let use_color = use_color_stdout();
    let store = ProfileStore::load(paths)?;
    let selected_ids = resolve_export_ids(paths, &store, label.as_deref(), &ids)?;
    let mut profiles = Vec::with_capacity(selected_ids.len());

    for id in selected_ids {
        let path = profile_path_for_id(&paths.profiles, &id);
        let raw = fs::read_to_string(&path)
            .map_err(|err| crate::msg2(PROFILE_ERR_READ_PROFILES_DIR, path.display(), err))?;
        let contents: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|err| format!("Error: Saved profile '{}' is invalid JSON: {err}", id))?;
        let managed_files =
            managed_files_for_profile(&paths.profiles, &id, store.profiles_index.profiles.get(&id));
        let config_toml = if managed_files_contains_config(&managed_files) {
            let config_path = profile_config_path_for_id(&paths.profiles, &id);
            Some(fs::read_to_string(&config_path).map_err(|err| {
                format!(
                    "Error: Saved profile '{}' is missing config.toml at {}: {err}",
                    id,
                    config_path.display()
                )
            })?)
        } else {
            None
        };
        profiles.push(ExportedProfile {
            label: label_for_id(&store.labels, &id),
            id,
            contents,
            managed_files,
            config_toml,
        });
    }

    let bundle = ExportBundle {
        version: 1,
        profiles,
    };
    let mut bytes = serde_json::to_vec_pretty(&bundle).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    crate::common::write_atomic_private(&output, &bytes)?;
    tighten_export_permissions(&output)?;

    let count = bundle.profiles.len();
    let noun = if count == 1 { "profile" } else { "profiles" };

    if json {
        let result = CommandResultJson::success(
            "export",
            serde_json::json!({
                "path": output.display().to_string(),
                "count": count,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let message = format_action(
        &format!("Exported {count} {noun} to {}", output.display()),
        use_color,
    );
    print_output_block(&message);
    Ok(())
}

pub(crate) fn import_profiles(paths: &Paths, input: PathBuf, json: bool) -> Result<(), String> {
    let use_color = use_color_stdout();
    let raw = fs::read_to_string(&input).map_err(|err| {
        format!(
            "Error: Could not read import file {}: {err}",
            input.display()
        )
    })?;
    let bundle: ExportBundle = serde_json::from_str(&raw)
        .map_err(|err| format!("Error: Import file is invalid JSON: {err}"))?;
    if bundle.version != 1 {
        return Err(format!(
            "Error: Import file version {} is not supported.",
            bundle.version
        ));
    }

    let mut store = ProfileStore::load(paths)?;
    let existing_ids = collect_profile_ids(&paths.profiles)?;
    let mut staged_labels = store.labels.clone();
    let mut seen_ids = HashSet::new();
    let mut prepared = Vec::with_capacity(bundle.profiles.len());
    for profile in bundle.profiles {
        validate_import_profile_id(&profile.id)?;
        if !seen_ids.insert(profile.id.clone()) {
            return Err(format!(
                "Error: Import bundle contains duplicate profile id '{}'.",
                profile.id
            ));
        }
        if existing_ids.contains(&profile.id) {
            return Err(format!("Error: Profile '{}' already exists.", profile.id));
        }
        if let Some(label) = profile.label.as_deref() {
            assign_label(&mut staged_labels, label, &profile.id)?;
        }
        prepared.push(prepare_import_profile(profile)?);
    }

    let mut written_ids = Vec::with_capacity(prepared.len());
    for profile in &prepared {
        if let Err(err) = ensure_profile_dir(&paths.profiles, &profile.id) {
            cleanup_imported_profiles(paths, &written_ids);
            return Err(err);
        }
        written_ids.push(profile.id.clone());
        let path = profile_path_for_id(&paths.profiles, &profile.id);
        if let Err(err) = crate::common::write_atomic_private(&path, &profile.contents) {
            cleanup_imported_profiles(paths, &written_ids);
            return Err(err);
        }
        if let Some(config_toml) = profile.config_toml.as_deref() {
            let config_path = profile_config_path_for_id(&paths.profiles, &profile.id);
            if let Err(err) = crate::common::write_atomic_private(&config_path, config_toml) {
                cleanup_imported_profiles(paths, &written_ids);
                return Err(err);
            }
        }
    }

    for profile in &prepared {
        if let Some(label) = profile.label.as_deref() {
            assign_label(&mut store.labels, label, &profile.id)?;
        }
        update_profiles_index_entry(
            &mut store.profiles_index,
            &profile.id,
            Some(&profile.tokens),
            profile.label.clone(),
            Some(profile.managed_files.clone()),
        );
    }
    if let Err(err) = store.save(paths) {
        cleanup_imported_profiles(paths, &written_ids);
        return Err(err);
    }

    let count = prepared.len();
    let noun = if count == 1 { "profile" } else { "profiles" };

    if json {
        let imported: Vec<serde_json::Value> = prepared
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "label": p.label,
                    "managed_files": p.managed_files,
                })
            })
            .collect();
        let result = CommandResultJson::success(
            "import",
            serde_json::json!({
                "count": count,
                "profiles": imported,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let message = format_action(
        &format!("Imported {count} {noun} from {}", input.display()),
        use_color,
    );
    print_output_block(&message);
    Ok(())
}

fn resolve_export_ids(
    paths: &Paths,
    store: &ProfileStore,
    label: Option<&str>,
    ids: &[String],
) -> Result<Vec<String>, String> {
    if let Some(label) = label {
        return Ok(vec![resolve_label_target_id(store, Some(label), None)?]);
    }

    let available_ids = collect_profile_ids(&paths.profiles)?;
    if ids.is_empty() {
        let mut all: Vec<String> = available_ids.into_iter().collect();
        all.sort();
        return Ok(all);
    }

    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for id in ids {
        if !available_ids.contains(id) {
            return Err(crate::msg2(
                PROFILE_ERR_ID_NO_MATCH,
                id,
                format_list_hint(use_color_stderr()),
            ));
        }
        if seen.insert(id.clone()) {
            selected.push(id.clone());
        }
    }
    Ok(selected)
}

fn prepare_import_profile(profile: ExportedProfile) -> Result<PreparedImportProfile, String> {
    let ExportedProfile {
        id,
        label,
        contents,
        managed_files,
        config_toml,
    } = profile;
    let mut managed_files = normalize_managed_files(managed_files);
    if config_toml.is_some() && !managed_files_contains_config(&managed_files) {
        managed_files.push(CONFIG_FILE_NAME.to_string());
        managed_files = normalize_managed_files(managed_files);
    }
    if managed_files_contains_config(&managed_files) && config_toml.is_none() {
        return Err(format!(
            "Error: Exported profile '{}' includes config.toml but no config_toml field.",
            id
        ));
    }
    let config_toml = config_toml.map(|contents| {
        let mut bytes = contents.into_bytes();
        if !bytes.ends_with(b"\n") {
            bytes.push(b'\n');
        }
        bytes
    });
    let mut bytes = serde_json::to_vec_pretty(&contents).map_err(|err| {
        format!(
            "Error: Exported profile '{}' could not be serialized: {err}",
            id
        )
    })?;
    bytes.push(b'\n');

    let auth: AuthFile = serde_json::from_value(contents)
        .map_err(|err| format!("Error: Exported profile '{}' is invalid JSON: {err}", id))?;
    let tokens = if let Some(tokens) = auth.tokens {
        tokens
    } else if let Some(api_key) = auth.openai_api_key.as_deref() {
        tokens_from_api_key(api_key)
    } else {
        return Err(format!(
            "Error: Exported profile '{}' is missing tokens or API key.",
            id
        ));
    };
    if !is_profile_ready(&tokens) {
        return Err(format!("Error: Exported profile '{}' is incomplete.", id));
    }

    Ok(PreparedImportProfile {
        id,
        label,
        contents: bytes,
        managed_files,
        config_toml,
        tokens,
    })
}

fn validate_import_profile_id(id: &str) -> Result<(), String> {
    let mut components = Path::new(id).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(format!("Error: Imported profile id '{}' is not safe.", id));
    }
    if matches!(id, "profiles" | "update") {
        return Err(format!("Error: Imported profile id '{}' is reserved.", id));
    }
    Ok(())
}

fn cleanup_imported_profiles(paths: &Paths, ids: &[String]) {
    for id in ids {
        let _ = fs::remove_file(profile_path_for_id(&paths.profiles, id));
        let _ = fs::remove_file(profile_config_path_for_id(&paths.profiles, id));
        let _ = remove_profile_dir_if_empty(&paths.profiles, id);
    }
}

fn tighten_export_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions).map_err(|err| {
            format!(
                "Error: Could not secure export file {}: {err}",
                path.display()
            )
        })?;
    }
    Ok(())
}
