use chrono::{DateTime, Local};
use colored::Colorize;
use inquire::{Confirm, MultiSelect, Select};
use rayon::prelude::*;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal as _};
use std::path::{Path, PathBuf};

use crate::json_response::CommandResultJson;
use crate::{
    AUTH_ERR_INCOMPLETE_ACCOUNT, AUTH_ERR_PROFILE_MISSING_EMAIL_PLAN, PROFILE_COPY_CONTEXT_LOAD,
    PROFILE_COPY_CONTEXT_SAVE, PROFILE_DELETE_HELP, PROFILE_ERR_COPY_CONTEXT,
    PROFILE_ERR_CURRENT_NOT_SAVED, PROFILE_ERR_DELETE_CONFIRM_REQUIRED, PROFILE_ERR_FAILED_DELETE,
    PROFILE_ERR_ID_NO_MATCH, PROFILE_ERR_LABEL_EMPTY, PROFILE_ERR_LABEL_NO_MATCH,
    PROFILE_ERR_PROMPT_CONTEXT, PROFILE_ERR_PROMPT_DELETE, PROFILE_ERR_PROMPT_LOAD,
    PROFILE_ERR_SELECTED_INVALID, PROFILE_ERR_SERIALIZE_INDEX, PROFILE_ERR_TTY_REQUIRED,
    PROFILE_LOAD_HELP, PROFILE_MSG_DELETED_COUNT, PROFILE_MSG_DELETED_WITH,
    PROFILE_MSG_LABEL_CLEARED, PROFILE_MSG_LABEL_SET, PROFILE_MSG_LOADED_WITH,
    PROFILE_MSG_NOT_FOUND, PROFILE_MSG_SAVED, PROFILE_MSG_SAVED_WITH, PROFILE_PROMPT_CANCEL,
    PROFILE_PROMPT_CONTINUE_WITHOUT_SAVING, PROFILE_PROMPT_DELETE_MANY, PROFILE_PROMPT_DELETE_ONE,
    PROFILE_PROMPT_DELETE_SELECTED, PROFILE_PROMPT_SAVE_AND_CONTINUE, PROFILE_SUMMARY_AUTH_ERROR,
    PROFILE_SUMMARY_ERROR, PROFILE_SUMMARY_FILE_MISSING, PROFILE_SUMMARY_USAGE_ERROR,
    PROFILE_WARN_CURRENT_NOT_SAVED_REASON, UI_ERROR_PREFIX, UI_ERROR_TWO_LINE,
};
use crate::{
    CANCELLED_MESSAGE, format_action, format_entry_header, format_error, format_label_later_hint,
    format_list_hint, format_no_profiles, format_save_before_load_or_force, format_unsaved_warning,
    format_warning, inquire_select_render_config, is_inquire_cancel, is_plain, normalize_error,
    print_output_block, style_text, use_color_stderr, use_color_stdout,
};
use crate::{
    Paths, USAGE_UNAVAILABLE_API_KEY_DETAIL, USAGE_UNAVAILABLE_API_KEY_TITLE, command_name,
    copy_atomic,
};
use crate::{
    Tokens, extract_email_and_plan, extract_profile_identity, is_api_key_profile, is_free_plan,
    is_profile_ready, profile_error, read_tokens, token_account_id,
};
use crate::{format_usage_unavailable, read_base_url, usage_unavailable};

mod identity;
mod import_export;
mod index;
mod labels;
mod managed_files;
mod paths;
mod store;

#[cfg(test)]
pub(crate) use identity::next_snowflake_profile_id;
pub(crate) use identity::{cached_profile_ids, pick_primary, resolve_save_id, resolve_sync_id};
pub(crate) use import_export::{export_profiles, import_profiles};
#[cfg(test)]
pub(crate) use index::PROFILES_INDEX_VERSION;
pub(crate) use index::{
    ProfileIndexEntry, ProfilesIndex, labels_from_index, prune_profiles_index, read_profiles_index,
    read_profiles_index_relaxed, repair_profiles_metadata, sync_profiles_index,
    update_profiles_index_entry, write_profiles_index,
};
pub(crate) use labels::{assign_label, label_for_id, remove_labels_for_id, resolve_label_id};
pub(crate) use paths::{profile_files, profile_id_from_path};
#[cfg(test)]
pub(crate) use store::load_profile_tokens_map;
pub(crate) use store::{
    ProfileStore, Snapshot, current_saved_id, load_snapshot, sync_current, sync_profile,
    unsaved_reason,
};

use labels::{labels_by_id, prune_labels, resolve_label_target_id};
use managed_files::*;
use paths::*;

const DEFAULT_USAGE_CONCURRENCY: usize = 32;
const MAX_USAGE_CONCURRENCY: usize = 128;
const USAGE_CONCURRENCY_ENV: &str = "CODEXSWITCH_CLI_USAGE_CONCURRENCY";
const LEGACY_USAGE_CONCURRENCY_ENV: &str = "CODEX_PROFILES_USAGE_CONCURRENCY";
const AUTH_FILE_NAME: &str = "auth.json";
const CONFIG_FILE_NAME: &str = "config.toml";
pub fn save_profile(
    paths: &Paths,
    label: Option<String>,
    include_config: bool,
    json: bool,
) -> Result<(), String> {
    let use_color = use_color_stdout();
    let mut store = ProfileStore::load(paths)?;
    let tokens = read_tokens(&paths.auth)?;
    let id = resolve_save_id(paths, &mut store.profiles_index, &tokens)?;
    let managed_files = managed_files_for_save(include_config);

    if let Some(label) = label.as_deref() {
        assign_label(&mut store.labels, label, &id)?;
    }

    let target = profile_path_for_id(&paths.profiles, &id);
    let config_target = profile_config_path_for_id(&paths.profiles, &id);
    if include_config && !paths.config.is_file() {
        return Err(format!(
            "Error: Config file not found: {}",
            paths.config.display()
        ));
    }
    if !include_config {
        remove_profile_config_if_present(&config_target)?;
    }
    ensure_profile_dir(&paths.profiles, &id)?;
    copy_profile(&paths.auth, &target, PROFILE_COPY_CONTEXT_SAVE)?;
    if include_config {
        copy_profile(&paths.config, &config_target, PROFILE_COPY_CONTEXT_SAVE)?;
    }

    let label_display = label_for_id(&store.labels, &id);
    update_profiles_index_entry(
        &mut store.profiles_index,
        &id,
        Some(&tokens),
        label_display.clone(),
        Some(managed_files.clone()),
    );
    store.save(paths)?;

    if json {
        let result = CommandResultJson::success(
            "save",
            serde_json::json!({
                "id": id,
                "label": label_display,
                "managed_files": managed_files,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let info = profile_info(Some(&tokens), label_display.clone(), true, use_color);
    let message = if info.email.is_some() {
        crate::msg1(PROFILE_MSG_SAVED_WITH, info.display)
    } else {
        PROFILE_MSG_SAVED.to_string()
    };
    let mut message = format_action(&message, use_color);
    message.push('\n');
    message.push_str(&format_managed_files_line(&managed_files, use_color));
    if label_display.is_none() {
        message.push('\n');
        message.push_str(&format_label_later_hint(&id, use_color));
    }
    print_output_block(&message);
    Ok(())
}

pub fn set_profile_label(
    paths: &Paths,
    label: Option<String>,
    id: Option<String>,
    to: String,
    json: bool,
) -> Result<(), String> {
    let use_color = use_color_stdout();
    let mut store = ProfileStore::load(paths)?;
    let target_id = resolve_label_target_id(&store, label.as_deref(), id.as_deref())?;
    let target_label = trim_label(&to)?.to_string();

    assign_label(&mut store.labels, &target_label, &target_id)?;
    store.save(paths)?;

    if json {
        let result = CommandResultJson::success(
            "label set",
            serde_json::json!({
                "id": target_id,
                "label": target_label,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let message = format_action(
        &crate::msg2(PROFILE_MSG_LABEL_SET, target_label, target_id),
        use_color,
    );
    print_output_block(&message);
    Ok(())
}

pub fn clear_profile_label(
    paths: &Paths,
    label: Option<String>,
    id: Option<String>,
    json: bool,
) -> Result<(), String> {
    let use_color = use_color_stdout();
    let mut store = ProfileStore::load(paths)?;
    let target_id = resolve_label_target_id(&store, label.as_deref(), id.as_deref())?;

    remove_labels_for_id(&mut store.labels, &target_id);
    store.save(paths)?;

    if json {
        let result = CommandResultJson::success(
            "label clear",
            serde_json::json!({
                "id": target_id,
                "label": null,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let message = format_action(
        &crate::msg1(PROFILE_MSG_LABEL_CLEARED, target_id),
        use_color,
    );
    print_output_block(&message);
    Ok(())
}

pub fn rename_profile_label(
    paths: &Paths,
    label: String,
    to: String,
    json: bool,
) -> Result<(), String> {
    let use_color = use_color_stdout();
    let mut store = ProfileStore::load(paths)?;
    let old_label = trim_label(&label)?.to_string();
    let target_id = resolve_label_id(&store.labels, &old_label)?;
    let new_label = trim_label(&to)?.to_string();

    assign_label(&mut store.labels, &new_label, &target_id)?;
    store.save(paths)?;

    if json {
        let result = CommandResultJson::success(
            "label rename",
            serde_json::json!({
                "id": target_id,
                "label": new_label,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let message = format_action(
        &format!("Renamed label '{}' to '{}'", old_label, new_label),
        use_color,
    );
    print_output_block(&message);
    Ok(())
}

pub fn load_profile(
    paths: &Paths,
    label: Option<String>,
    id: Option<String>,
    force: bool,
    json: bool,
) -> Result<(), String> {
    let use_color_err = use_color_stderr();
    let use_color_out = use_color_stdout();
    let no_profiles = format_no_profiles(paths, use_color_err);
    let (mut snapshot, mut ordered) = load_snapshot_ordered(paths, true, &no_profiles)?;

    if let Some(reason) = unsaved_reason(paths, &snapshot.tokens)
        && !force
    {
        match prompt_unsaved_load(paths, &reason)? {
            LoadChoice::SaveAndContinue => {
                save_profile(paths, None, false, false)?;
                let no_profiles = format_no_profiles(paths, use_color_err);
                let result = load_snapshot_ordered(paths, true, &no_profiles)?;
                snapshot = result.0;
                ordered = result.1;
            }
            LoadChoice::ContinueWithoutSaving => {}
            LoadChoice::Cancel => {
                return Err(CANCELLED_MESSAGE.to_string());
            }
        }
    }

    let candidates = make_candidates(paths, &snapshot, &ordered);
    let selected = pick_one(
        "load",
        label.as_deref(),
        id.as_deref(),
        &snapshot,
        &candidates,
    )?;
    let selected_id = selected.id.clone();
    let selected_display = selected.display.clone();

    match snapshot.tokens.get(&selected_id) {
        Some(Ok(_)) => {}
        Some(Err(err)) => {
            let message = err
                .strip_prefix(&format!("{} ", UI_ERROR_PREFIX))
                .unwrap_or(err);
            return Err(crate::msg1(PROFILE_ERR_SELECTED_INVALID, message));
        }
        None => {
            return Err(profile_not_found(use_color_err));
        }
    }

    let mut store = ProfileStore::load(paths)?;

    if let Err(err) = sync_current(paths, &mut store.profiles_index) {
        let warning = format_warning(&err, use_color_err);
        eprintln!("{warning}");
    }

    let source = profile_path_for_id(&paths.profiles, &selected_id);
    if !source.is_file() {
        return Err(profile_not_found(use_color_err));
    }
    let managed_files = managed_files_for_profile(
        &paths.profiles,
        &selected_id,
        store.profiles_index.profiles.get(&selected_id),
    );
    if managed_files_contains_config(&managed_files) {
        let source_config = profile_config_path_for_id(&paths.profiles, &selected_id);
        if !source_config.is_file() {
            return Err(format!(
                "Error: Saved profile '{}' includes config.toml but {} is missing.",
                selected_id,
                source_config.display()
            ));
        }
    }

    copy_profile(&source, &paths.auth, PROFILE_COPY_CONTEXT_LOAD)?;
    if managed_files_contains_config(&managed_files) {
        let source_config = profile_config_path_for_id(&paths.profiles, &selected_id);
        copy_profile(&source_config, &paths.config, PROFILE_COPY_CONTEXT_LOAD)?;
    }

    let label = label_for_id(&store.labels, &selected_id);
    let tokens = snapshot
        .tokens
        .get(&selected_id)
        .and_then(|result| result.as_ref().ok());
    update_profiles_index_entry(
        &mut store.profiles_index,
        &selected_id,
        tokens,
        label.clone(),
        Some(managed_files.clone()),
    );
    store.save(paths)?;

    if json {
        let result = CommandResultJson::success(
            "load",
            serde_json::json!({
                "id": selected_id,
                "label": label,
                "managed_files": managed_files,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let mut message = format_action(
        &crate::msg1(PROFILE_MSG_LOADED_WITH, selected_display),
        use_color_out,
    );
    message.push('\n');
    message.push_str(&format_managed_files_line(&managed_files, use_color_out));
    print_output_block(&message);
    Ok(())
}

pub fn delete_profile(
    paths: &Paths,
    yes: bool,
    label: Option<String>,
    ids: Vec<String>,
    json: bool,
) -> Result<(), String> {
    let use_color_out = use_color_stdout();
    let use_color_err = use_color_stderr();
    let no_profiles = format_no_profiles(paths, use_color_out);
    let (snapshot, ordered) = match load_snapshot_ordered(paths, true, &no_profiles) {
        Ok(result) => result,
        Err(message) => {
            if message == no_profiles {
                print_output_block(&message);
                return Ok(());
            }
            return Err(message);
        }
    };

    let candidates = make_candidates(paths, &snapshot, &ordered);
    let selections = pick_many("delete", label.as_deref(), &ids, &snapshot, &candidates)?;
    let (selected_ids, displays): (Vec<String>, Vec<String>) = selections
        .iter()
        .map(|item| (item.id.clone(), item.display.clone()))
        .unzip();

    if selected_ids.is_empty() {
        return Ok(());
    }

    let mut store = ProfileStore::load(paths)?;
    if !yes && !confirm_delete_profiles(&displays)? {
        return Err(CANCELLED_MESSAGE.to_string());
    }

    for selected in &selected_ids {
        let target = profile_path_for_id(&paths.profiles, selected);
        if !target.is_file() {
            return Err(profile_not_found(use_color_err));
        }
        fs::remove_file(&target).map_err(|err| crate::msg1(PROFILE_ERR_FAILED_DELETE, err))?;
        remove_profile_config_if_present(&profile_config_path_for_id(&paths.profiles, selected))
            .map_err(|err| crate::msg1(PROFILE_ERR_FAILED_DELETE, err))?;
        remove_profile_dir_if_empty(&paths.profiles, selected)
            .map_err(|err| crate::msg1(PROFILE_ERR_FAILED_DELETE, err))?;
        remove_labels_for_id(&mut store.labels, selected);
        store.profiles_index.profiles.remove(selected);
    }
    store.save(paths)?;

    if json {
        let deleted: Vec<serde_json::Value> = selected_ids
            .iter()
            .zip(displays.iter())
            .map(|(id, display)| serde_json::json!({ "id": id, "display": display }))
            .collect();
        let result = CommandResultJson::success(
            "delete",
            serde_json::json!({
                "count": selected_ids.len(),
                "deleted": deleted,
            }),
        );
        result.print()?;
        return Ok(());
    }

    let message = if selected_ids.len() == 1 {
        crate::msg1(PROFILE_MSG_DELETED_WITH, &displays[0])
    } else {
        crate::msg1(PROFILE_MSG_DELETED_COUNT, selected_ids.len())
    };
    let message = format_action(&message, use_color_out);
    print_output_block(&message);
    Ok(())
}

pub fn list_profiles(paths: &Paths, json: bool, show_id: bool) -> Result<(), String> {
    let snapshot = load_snapshot(paths, false)?;
    let current_saved_id = current_saved_id(paths, &snapshot.tokens);
    let ctx = ListCtx::new(paths, false, true, show_id);

    let ordered = ordered_profile_ids(&snapshot, current_saved_id.as_deref());
    let current_entry = make_current(
        paths,
        current_saved_id.as_deref(),
        &snapshot.labels,
        &snapshot.tokens,
        &snapshot.index,
        &ctx,
    );
    let has_saved = !ordered.is_empty();
    if !has_saved {
        if json {
            if let Some(entry) = current_entry {
                return print_list_json(&[entry]);
            }
            return print_list_json(&[]);
        }
        if let Some(entry) = current_entry {
            let lines = render_entries(&[entry], &ctx, false);
            print_output_block(&lines.join("\n"));
        } else {
            let message = format_no_profiles(paths, ctx.use_color);
            print_output_block(&message);
        }
        return Ok(());
    }

    let filtered: Vec<String> = ordered
        .into_iter()
        .filter(|id| current_saved_id.as_deref() != Some(id.as_str()))
        .collect();
    let list_entries = make_entries(&filtered, &snapshot, None, &ctx);

    if json {
        let mut entries = Vec::new();
        if let Some(entry) = current_entry {
            entries.push(entry);
        }
        entries.extend(list_entries);
        return print_list_json(&entries);
    }

    let mut lines = Vec::new();
    if let Some(entry) = current_entry.as_ref() {
        lines.extend(render_entries(std::slice::from_ref(entry), &ctx, false));
        if !list_entries.is_empty() {
            push_separator(&mut lines, false);
        }
    }
    lines.extend(render_entries(&list_entries, &ctx, false));
    let output = lines.join("\n");
    print_output_block(&output);
    Ok(())
}

pub fn status_profiles(
    paths: &Paths,
    all: bool,
    label: Option<String>,
    id: Option<String>,
    json: bool,
) -> Result<(), String> {
    if all {
        return status_all_profiles(paths, json);
    }

    if label.is_some() || id.is_some() {
        return status_selected_profile(paths, label.as_deref(), id.as_deref(), json);
    }

    let snapshot = load_snapshot(paths, false)?;
    let current_saved_id = current_saved_id(paths, &snapshot.tokens);
    let mut ctx = ListCtx::new(paths, true, false, false);
    if json {
        ctx.use_color = false;
    }
    let labels = &snapshot.labels;
    let tokens_map = &snapshot.tokens;
    let current_entry = make_current(
        paths,
        current_saved_id.as_deref(),
        labels,
        tokens_map,
        &snapshot.index,
        &ctx,
    );
    if json {
        return print_current_status_json(current_entry);
    }
    if let Some(entry) = current_entry {
        let lines = render_entries(&[entry], &ctx, false);
        print_output_block(&lines.join("\n"));
    } else {
        let message = format_no_profiles(paths, ctx.use_color);
        print_output_block(&message);
    }
    Ok(())
}

fn status_selected_profile(
    paths: &Paths,
    label: Option<&str>,
    id: Option<&str>,
    json: bool,
) -> Result<(), String> {
    let use_color = use_color_stdout();
    let no_profiles = format_no_profiles(paths, use_color);
    let (snapshot, ordered) = match load_snapshot_ordered(paths, false, &no_profiles) {
        Ok(result) => result,
        Err(message) => {
            if message == no_profiles {
                if json {
                    return print_current_status_json(None);
                }
                print_output_block(&message);
                return Ok(());
            }
            return Err(message);
        }
    };
    let current_saved_id = current_saved_id(paths, &snapshot.tokens);
    let mut ctx = ListCtx::new(paths, true, false, false);
    if json {
        ctx.use_color = false;
    }

    let candidates = build_candidates(&ordered, &snapshot, current_saved_id.as_deref());
    let selected = if let Some(label) = label {
        select_by_label(label, &snapshot.labels, &candidates)?
    } else if let Some(id) = id {
        select_by_id(id, &candidates)?
    } else {
        unreachable!("status selector requires label or id")
    };

    let mut entries = make_entries(
        std::slice::from_ref(&selected.id),
        &snapshot,
        current_saved_id.as_deref(),
        &ctx,
    );
    let Some(entry) = entries.pop() else {
        return Err(profile_not_found(use_color_stderr()));
    };

    if json {
        return print_current_status_json(Some(entry));
    }

    let lines = render_entries(&[entry], &ctx, false);
    print_output_block(&lines.join("\n"));
    Ok(())
}

fn status_all_profiles(paths: &Paths, json: bool) -> Result<(), String> {
    let snapshot = load_snapshot(paths, false)?;
    let current_saved_id = current_saved_id(paths, &snapshot.tokens);
    let mut ctx = ListCtx::new(paths, true, true, false);
    if json {
        ctx.use_color = false;
    }

    let ordered = ordered_profile_ids(&snapshot, current_saved_id.as_deref());
    let filtered: Vec<String> = ordered
        .into_iter()
        .filter(|id| current_saved_id.as_deref() != Some(id.as_str()))
        .collect();

    let (current_entry, list_entries) = rayon::join(
        || {
            make_current(
                paths,
                current_saved_id.as_deref(),
                &snapshot.labels,
                &snapshot.tokens,
                &snapshot.index,
                &ctx,
            )
        },
        || make_entries(&filtered, &snapshot, None, &ctx),
    );

    if json {
        let mut profiles = Vec::new();
        if let Some(entry) = current_entry {
            profiles.push(entry);
        }
        profiles.extend(list_entries);
        return print_all_status_json(profiles);
    }

    if current_entry.is_none() && list_entries.is_empty() {
        let message = format_no_profiles(paths, ctx.use_color);
        print_output_block(&message);
        return Ok(());
    }

    let mut lines = Vec::new();
    if let Some(err) = ctx.base_url_error.as_deref() {
        lines.push(format_error(err));
        if current_entry.is_some() || !list_entries.is_empty() {
            push_separator(&mut lines, true);
        }
    }
    if let Some(entry) = current_entry {
        lines.extend(render_entries(&[entry], &ctx, true));
        if !list_entries.is_empty() {
            push_separator(&mut lines, true);
            lines.push(String::new());
        }
    }

    if !list_entries.is_empty() {
        lines.extend(render_entries(&list_entries, &ctx, true));
    }

    let output = lines.join("\n");
    print_output_block(&output);
    Ok(())
}

pub type Labels = BTreeMap<String, String>;

fn profile_not_found(use_color: bool) -> String {
    crate::msg1(PROFILE_MSG_NOT_FOUND, format_list_hint(use_color))
}

fn load_snapshot_ordered(
    paths: &Paths,
    strict_labels: bool,
    no_profiles_message: &str,
) -> Result<(Snapshot, Vec<String>), String> {
    let snapshot = load_snapshot(paths, strict_labels)?;
    let current_saved = current_saved_id(paths, &snapshot.tokens);
    let ordered = ordered_profile_ids(&snapshot, current_saved.as_deref());
    if ordered.is_empty() {
        return Err(no_profiles_message.to_string());
    }
    Ok((snapshot, ordered))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProfileOrderKey {
    current_rank: u8,
    label_missing: bool,
    label: String,
    email_missing: bool,
    email: String,
    id: String,
}

fn ordered_profile_ids(snapshot: &Snapshot, current_saved_id: Option<&str>) -> Vec<String> {
    let labels_by_id = labels_by_id(&snapshot.labels);
    let mut keyed: Vec<(String, ProfileOrderKey)> = snapshot
        .tokens
        .keys()
        .cloned()
        .map(|id| {
            let label = labels_by_id
                .get(&id)
                .cloned()
                .or_else(|| {
                    snapshot
                        .index
                        .profiles
                        .get(&id)
                        .and_then(|entry| entry.label.clone())
                })
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
                .unwrap_or_default();
            let email = snapshot
                .tokens
                .get(&id)
                .and_then(|result| result.as_ref().ok())
                .and_then(|tokens| extract_email_and_plan(tokens).0)
                .or_else(|| {
                    snapshot
                        .index
                        .profiles
                        .get(&id)
                        .and_then(|entry| entry.email.clone())
                })
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
                .unwrap_or_default();
            let key = ProfileOrderKey {
                current_rank: if current_saved_id == Some(id.as_str()) {
                    0
                } else {
                    1
                },
                label_missing: label.is_empty(),
                label,
                email_missing: email.is_empty(),
                email,
                id: id.to_ascii_lowercase(),
            };
            (id, key)
        })
        .collect();
    keyed.sort_by(|left, right| left.1.cmp(&right.1));
    keyed.into_iter().map(|(id, _)| id).collect()
}

fn copy_profile(source: &Path, dest: &Path, context: &str) -> Result<(), String> {
    copy_atomic(source, dest)
        .map_err(|err| crate::msg3(PROFILE_ERR_COPY_CONTEXT, context, dest.display(), err))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dest, fs::Permissions::from_mode(0o600))
            .map_err(|err| crate::msg3(PROFILE_ERR_COPY_CONTEXT, context, dest.display(), err))?;
    }
    Ok(())
}

fn make_candidates(paths: &Paths, snapshot: &Snapshot, ordered: &[String]) -> Vec<Candidate> {
    let current_saved = current_saved_id(paths, &snapshot.tokens);
    build_candidates(ordered, snapshot, current_saved.as_deref())
}

fn pick_one(
    action: &str,
    label: Option<&str>,
    id: Option<&str>,
    snapshot: &Snapshot,
    candidates: &[Candidate],
) -> Result<Candidate, String> {
    if let Some(label) = label {
        select_by_label(label, &snapshot.labels, candidates)
    } else if let Some(id) = id {
        select_by_id(id, candidates)
    } else if !io::stdin().is_terminal() {
        require_tty(action)?;
        unreachable!("require_tty should always return Err in non-interactive mode")
    } else {
        select_single_profile("", candidates)
    }
}

fn pick_many(
    action: &str,
    label: Option<&str>,
    ids: &[String],
    snapshot: &Snapshot,
    candidates: &[Candidate],
) -> Result<Vec<Candidate>, String> {
    if let Some(label) = label {
        Ok(vec![select_by_label(label, &snapshot.labels, candidates)?])
    } else if !ids.is_empty() {
        select_many_by_id(ids, candidates)
    } else {
        require_tty(action)?;
        select_multiple_profiles("", candidates)
    }
}

pub(crate) struct ProfileInfo {
    pub(crate) display: String,
    pub(crate) email: Option<String>,
    pub(crate) plan: Option<String>,
    pub(crate) is_free: bool,
}

pub(crate) fn profile_info(
    tokens: Option<&Tokens>,
    label: Option<String>,
    is_current: bool,
    use_color: bool,
) -> ProfileInfo {
    profile_info_with_fallback(tokens, None, label, is_current, use_color)
}

fn profile_info_with_fallback(
    tokens: Option<&Tokens>,
    fallback: Option<&ProfileIndexEntry>,
    label: Option<String>,
    is_current: bool,
    use_color: bool,
) -> ProfileInfo {
    let (email, plan) = if let Some(tokens) = tokens {
        extract_email_and_plan(tokens)
    } else if let Some(entry) = fallback {
        (entry.email.clone(), entry.plan.clone())
    } else {
        (None, None)
    };
    let is_free = is_free_plan(plan.as_deref());
    let display =
        crate::format_profile_display(email.clone(), plan.clone(), label, is_current, use_color);
    ProfileInfo {
        display,
        email,
        plan,
        is_free,
    }
}

#[derive(Debug)]
pub(crate) enum LoadChoice {
    SaveAndContinue,
    ContinueWithoutSaving,
    Cancel,
}

pub(crate) fn prompt_unsaved_load(paths: &Paths, reason: &str) -> Result<LoadChoice, String> {
    let is_tty = io::stdin().is_terminal();
    if !is_tty {
        let hint = format_save_before_load_or_force(paths, use_color_stderr());
        return Err(crate::msg1(PROFILE_ERR_CURRENT_NOT_SAVED, hint));
    }
    let selection = Select::new(
        "",
        vec![
            PROFILE_PROMPT_SAVE_AND_CONTINUE,
            PROFILE_PROMPT_CONTINUE_WITHOUT_SAVING,
            PROFILE_PROMPT_CANCEL,
        ],
    )
    .with_render_config(inquire_select_render_config())
    .prompt();
    prompt_unsaved_load_with(paths, reason, is_tty, selection)
}

fn prompt_unsaved_load_with(
    paths: &Paths,
    reason: &str,
    is_tty: bool,
    selection: Result<&str, inquire::error::InquireError>,
) -> Result<LoadChoice, String> {
    if !is_tty {
        let hint = format_save_before_load_or_force(paths, use_color_stderr());
        return Err(crate::msg1(PROFILE_ERR_CURRENT_NOT_SAVED, hint));
    }
    let warning = format_warning(
        &crate::msg1(PROFILE_WARN_CURRENT_NOT_SAVED_REASON, reason),
        use_color_stderr(),
    );
    eprintln!("{warning}");
    match selection {
        Ok(PROFILE_PROMPT_SAVE_AND_CONTINUE) => Ok(LoadChoice::SaveAndContinue),
        Ok(PROFILE_PROMPT_CONTINUE_WITHOUT_SAVING) => Ok(LoadChoice::ContinueWithoutSaving),
        Ok(_) => Ok(LoadChoice::Cancel),
        Err(err) if is_inquire_cancel(&err) => Ok(LoadChoice::Cancel),
        Err(err) => Err(crate::msg1(PROFILE_ERR_PROMPT_LOAD, err)),
    }
}

pub(crate) fn build_candidates(
    ordered: &[String],
    snapshot: &Snapshot,
    current_saved_id: Option<&str>,
) -> Vec<Candidate> {
    let mut candidates = Vec::with_capacity(ordered.len());
    let use_color = use_color_stderr();
    let labels_by_id = labels_by_id(&snapshot.labels);
    for id in ordered {
        let label = labels_by_id.get(id).cloned();
        let tokens = snapshot
            .tokens
            .get(id)
            .and_then(|result| result.as_ref().ok());
        let index_entry = snapshot.index.profiles.get(id);
        let is_current = current_saved_id == Some(id.as_str());
        let info = profile_info_with_fallback(tokens, index_entry, label, is_current, use_color);
        let marker = if is_current {
            current_profile_marker(use_color)
        } else {
            String::new()
        };
        candidates.push(Candidate {
            id: id.clone(),
            display: format!("{}{}", info.display, marker),
        });
    }
    candidates
}

pub(crate) fn require_tty(action: &str) -> Result<(), String> {
    require_tty_with(io::stdin().is_terminal(), action)
}

fn require_tty_with(is_tty: bool, action: &str) -> Result<(), String> {
    if is_tty {
        Ok(())
    } else {
        Err(crate::msg3(
            PROFILE_ERR_TTY_REQUIRED,
            action,
            command_name(),
            action,
        ))
    }
}

pub(crate) fn select_single_profile(
    title: &str,
    candidates: &[Candidate],
) -> Result<Candidate, String> {
    let options = candidates.to_vec();
    let render_config = inquire_select_render_config();
    let prompt = Select::new(title, options)
        .with_help_message(PROFILE_LOAD_HELP)
        .with_render_config(render_config)
        .prompt();
    handle_inquire_result(prompt, "selection")
}

pub(crate) fn select_multiple_profiles(
    title: &str,
    candidates: &[Candidate],
) -> Result<Vec<Candidate>, String> {
    let options = candidates.to_vec();
    let render_config = inquire_select_render_config();
    let prompt = MultiSelect::new(title, options)
        .with_help_message(PROFILE_DELETE_HELP)
        .with_render_config(render_config)
        .prompt();
    let selections = handle_inquire_result(prompt, "selection")?;
    if selections.is_empty() {
        return Err(CANCELLED_MESSAGE.to_string());
    }
    Ok(selections)
}

pub(crate) fn select_by_label(
    label: &str,
    labels: &Labels,
    candidates: &[Candidate],
) -> Result<Candidate, String> {
    let id = resolve_label_id(labels, label)?;
    let Some(candidate) = candidates.iter().find(|candidate| candidate.id == id) else {
        return Err(crate::msg2(
            PROFILE_ERR_LABEL_NO_MATCH,
            label,
            format_list_hint(use_color_stderr()),
        ));
    };
    Ok(candidate.clone())
}

pub(crate) fn select_by_id(id: &str, candidates: &[Candidate]) -> Result<Candidate, String> {
    let Some(candidate) = candidates.iter().find(|candidate| candidate.id == id) else {
        return Err(crate::msg2(
            PROFILE_ERR_ID_NO_MATCH,
            id,
            format_list_hint(use_color_stderr()),
        ));
    };
    Ok(candidate.clone())
}

fn select_many_by_id(ids: &[String], candidates: &[Candidate]) -> Result<Vec<Candidate>, String> {
    let mut selections = Vec::with_capacity(ids.len());
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        selections.push(select_by_id(id, candidates)?);
    }
    Ok(selections)
}

pub(crate) fn confirm_delete_profiles(displays: &[String]) -> Result<bool, String> {
    let is_tty = io::stdin().is_terminal();
    if !is_tty {
        return Err(PROFILE_ERR_DELETE_CONFIRM_REQUIRED.to_string());
    }
    let prompt = if displays.len() == 1 {
        crate::msg1(PROFILE_PROMPT_DELETE_ONE, &displays[0])
    } else {
        let count = displays.len();
        eprintln!("{}", crate::msg1(PROFILE_PROMPT_DELETE_MANY, count));
        for display in displays {
            eprintln!(" - {display}");
        }
        PROFILE_PROMPT_DELETE_SELECTED.to_string()
    };
    let selection = Confirm::new(&prompt)
        .with_default(false)
        .with_render_config(inquire_select_render_config())
        .prompt();
    confirm_delete_profiles_with(is_tty, selection)
}

fn confirm_delete_profiles_with(
    is_tty: bool,
    selection: Result<bool, inquire::error::InquireError>,
) -> Result<bool, String> {
    if !is_tty {
        return Err(PROFILE_ERR_DELETE_CONFIRM_REQUIRED.to_string());
    }
    match selection {
        Ok(value) => Ok(value),
        Err(err) if is_inquire_cancel(&err) => Err(CANCELLED_MESSAGE.to_string()),
        Err(err) => Err(crate::msg1(PROFILE_ERR_PROMPT_DELETE, err)),
    }
}

#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) id: String,
    pub(crate) display: String,
}

impl fmt::Display for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = format_entry_header(&self.display, use_color_stderr());
        write!(f, "{header}")
    }
}

fn render_entries(entries: &[Entry], ctx: &ListCtx, allow_plain_spacing: bool) -> Vec<String> {
    let mut lines = Vec::with_capacity((entries.len().max(1)) * 4);
    for (idx, entry) in entries.iter().enumerate() {
        let mut entry_lines = Vec::new();
        let mut header = format_entry_header(&entry.display, ctx.use_color);
        if ctx.show_id
            && let Some(id) = entry.id.as_deref()
        {
            header.push_str(&format_profile_id_suffix(id, ctx.use_color));
        }
        if ctx.show_current_marker && entry.is_current {
            header.push_str(&current_profile_marker(ctx.use_color));
        }
        header.push_str(&format_managed_files_suffix(
            &entry.managed_files,
            ctx.use_color,
        ));
        let show_detail_lines = ctx.show_usage || entry.always_show_details;
        if !show_detail_lines {
            if let Some(err) = entry.error_summary.as_deref() {
                header.push_str(&format!("  {err}"));
                entry_lines.push(header);
            } else {
                entry_lines.push(header);
            }
        } else {
            entry_lines.push(header);
            entry_lines.push(String::new());
            entry_lines.extend(entry.details.iter().flat_map(|line| {
                if line.is_empty() {
                    vec![String::new()]
                } else {
                    line.lines()
                        .enumerate()
                        .map(|(index, part)| {
                            if part.is_empty() {
                                String::new()
                            } else if index == 0 {
                                format!(" {part}")
                            } else {
                                part.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                }
            }));
        }
        lines.extend(entry_lines);
        if idx + 1 < entries.len() {
            push_separator(&mut lines, allow_plain_spacing);
            if ctx.show_usage && allow_plain_spacing {
                lines.push(String::new());
            }
        }
    }
    lines
}

fn push_separator(lines: &mut Vec<String>, allow_plain_spacing: bool) {
    if !is_plain() || allow_plain_spacing {
        lines.push(String::new());
    }
}

fn current_profile_marker(use_color: bool) -> String {
    style_text(" <- active", use_color, |text| text.dimmed().italic())
}

fn format_profile_id_suffix(id: &str, use_color: bool) -> String {
    style_text(&format!(" [id: {id}]"), use_color, |text| text.dimmed())
}

fn make_error(
    id: Option<String>,
    label: Option<String>,
    index_entry: Option<&ProfileIndexEntry>,
    managed_files: Vec<String>,
    use_color: bool,
    message: &str,
    is_current: bool,
) -> Entry {
    let info = profile_info_with_fallback(None, index_entry, label.clone(), is_current, use_color);
    let is_saved = id.is_some();
    Entry {
        id,
        label,
        email: info.email.clone(),
        plan: info.plan.clone(),
        is_api_key: index_entry.map(|entry| entry.is_api_key).unwrap_or(false),
        is_saved,
        managed_files,
        display: info.display,
        details: vec![format_error(message)],
        warnings: Vec::new(),
        usage: None,
        error_summary: Some(error_summary(PROFILE_SUMMARY_ERROR, message)),
        always_show_details: false,
        is_current,
    }
}

fn unavailable_lines(message: &str, use_color: bool) -> Vec<String> {
    let (summary, detail) = usage_message_parts(message);
    let mut lines = vec![format_usage_unavailable(&summary, use_color)];
    if let Some(detail) = detail {
        lines.extend(
            detail
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| format!("      {line}")),
        );
    }
    lines
}

fn plain_error_lines(message: &str, use_color: bool) -> Vec<String> {
    let mut lines = message.lines();
    let Some(first) = lines.next() else {
        return Vec::new();
    };

    let mut headline = first.to_string();
    let mut tail: Vec<String> = lines.map(str::to_string).collect();
    let mut merged_status = false;
    if let Some(second) = tail.first() {
        let second = second.trim();
        if second.starts_with("unexpected status ") {
            headline = format!("{headline} ({second})");
            tail.remove(0);
            merged_status = true;
        }
    }

    let mut rendered = vec![format_error(&headline)];
    rendered.extend(tail.into_iter().enumerate().map(|(index, line)| {
        let adjusted_index = if merged_status { index + 1 } else { index };
        let text = if adjusted_index == 0 {
            line
        } else {
            format!(" {line}")
        };
        if adjusted_index == 0 {
            text
        } else {
            crate::ui::style_text(&text, use_color, |text| text.dimmed())
        }
    }));
    rendered
}

fn usage_message_parts(message: &str) -> (String, Option<String>) {
    let normalized = normalize_error(message);
    let mut lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    let summary = lines.next().unwrap_or_default().to_string();
    let detail_lines: Vec<&str> = lines.collect();
    let detail = if detail_lines.is_empty() {
        None
    } else {
        Some(detail_lines.join("\n"))
    };
    (summary, detail)
}

#[derive(Clone, Serialize)]
struct StatusUsageJson {
    state: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    buckets: Vec<crate::usage::UsageSnapshotBucket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl StatusUsageJson {
    fn ok(buckets: Vec<crate::usage::UsageSnapshotBucket>) -> Self {
        Self {
            state: "ok",
            buckets,
            status_code: None,
            summary: None,
            detail: None,
        }
    }

    fn from_message(state: &'static str, status_code: Option<u16>, message: &str) -> Self {
        let (summary, detail) = usage_message_parts(message);
        Self {
            state,
            buckets: Vec::new(),
            status_code,
            summary: Some(summary),
            detail,
        }
    }

    fn from_fetch_error(err: &crate::usage::UsageFetchError) -> Self {
        Self::from_message("error", err.status_code(), &err.message())
    }

    fn unavailable(message: &str) -> Self {
        Self::from_message("unavailable", None, message)
    }
}

fn detail_lines(
    tokens: &mut Tokens,
    email: Option<&str>,
    plan: Option<&str>,
    ctx: &ListCtx,
    source_path: &Path,
) -> (Vec<String>, Option<String>, Option<StatusUsageJson>, bool) {
    let use_color = ctx.use_color;
    let initial_account_id = token_account_id(tokens).map(str::to_string);
    let access_token = tokens.access_token.clone();
    if is_api_key_profile(tokens) {
        if ctx.show_usage {
            let message = crate::msg2(
                UI_ERROR_TWO_LINE,
                USAGE_UNAVAILABLE_API_KEY_TITLE,
                USAGE_UNAVAILABLE_API_KEY_DETAIL,
            );
            return (
                vec![format_error(&message)],
                None,
                Some(StatusUsageJson::unavailable(&message)),
                false,
            );
        }
        return (Vec::new(), None, None, false);
    }
    let unavailable_text = usage_unavailable();
    if let Some(message) = profile_error(tokens, email, plan) {
        let missing_access = access_token.is_none() || initial_account_id.is_none();
        let missing_identity_only =
            message == AUTH_ERR_PROFILE_MISSING_EMAIL_PLAN && !missing_access;
        if !missing_identity_only {
            if ctx.show_usage && missing_access && email.is_some() && plan.is_some() {
                return (
                    unavailable_lines(unavailable_text, use_color),
                    None,
                    Some(StatusUsageJson::unavailable(unavailable_text)),
                    false,
                );
            }
            let details = vec![format_error(message)];
            let summary = Some(error_summary(PROFILE_SUMMARY_ERROR, message));
            return (
                details,
                summary,
                Some(StatusUsageJson::from_message("error", None, message)),
                false,
            );
        }
    }
    if ctx.show_usage {
        if let Some(err) = ctx.base_url_error.as_deref() {
            return (
                vec![format_error(err)],
                Some(error_summary(PROFILE_SUMMARY_USAGE_ERROR, err)),
                Some(StatusUsageJson::from_message("error", None, err)),
                false,
            );
        }
        let Some(base_url) = ctx.base_url.as_deref() else {
            return (Vec::new(), None, None, false);
        };
        let Some(access_token) = access_token.as_deref() else {
            return (Vec::new(), None, None, false);
        };
        let Some(account_id) = initial_account_id.as_deref() else {
            return (Vec::new(), None, None, false);
        };
        match crate::usage::fetch_usage_status(
            base_url,
            access_token,
            account_id,
            unavailable_text,
            ctx.now,
        ) {
            Ok((details, buckets)) => (details, None, Some(StatusUsageJson::ok(buckets)), false),
            Err(err) if err.status_code() == Some(401) => {
                match crate::auth::refresh_profile_tokens(source_path, tokens) {
                    Ok(()) => {
                        let Some(access_token) = tokens.access_token.as_deref() else {
                            let message = AUTH_ERR_INCOMPLETE_ACCOUNT;
                            return (
                                vec![format_error(message)],
                                Some(error_summary(PROFILE_SUMMARY_AUTH_ERROR, message)),
                                Some(StatusUsageJson::from_message("error", None, message)),
                                true,
                            );
                        };
                        let Some(account_id) =
                            token_account_id(tokens).or(initial_account_id.as_deref())
                        else {
                            let message = AUTH_ERR_INCOMPLETE_ACCOUNT;
                            return (
                                vec![format_error(message)],
                                Some(error_summary(PROFILE_SUMMARY_AUTH_ERROR, message)),
                                Some(StatusUsageJson::from_message("error", None, message)),
                                true,
                            );
                        };
                        match crate::usage::fetch_usage_status(
                            base_url,
                            access_token,
                            account_id,
                            unavailable_text,
                            ctx.now,
                        ) {
                            Ok((details, buckets)) => {
                                (details, None, Some(StatusUsageJson::ok(buckets)), true)
                            }
                            Err(err) if err.status_code() == Some(401) => (
                                plain_error_lines(&err.plain_message(), use_color),
                                Some(error_summary(PROFILE_SUMMARY_AUTH_ERROR, &err.message())),
                                Some(StatusUsageJson::from_fetch_error(&err)),
                                true,
                            ),
                            Err(err) => (
                                plain_error_lines(&err.plain_message(), use_color),
                                Some(error_summary(PROFILE_SUMMARY_USAGE_ERROR, &err.message())),
                                Some(StatusUsageJson::from_fetch_error(&err)),
                                true,
                            ),
                        }
                    }
                    Err(err) => (
                        vec![format_error(&err)],
                        Some(error_summary(PROFILE_SUMMARY_AUTH_ERROR, &err)),
                        Some(StatusUsageJson::from_message("error", None, &err)),
                        false,
                    ),
                }
            }
            Err(err) => (
                plain_error_lines(&err.plain_message(), use_color),
                Some(error_summary(PROFILE_SUMMARY_USAGE_ERROR, &err.message())),
                Some(StatusUsageJson::from_fetch_error(&err)),
                false,
            ),
        }
    } else {
        (Vec::new(), None, None, false)
    }
}

#[cfg(test)]
fn is_http_401_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("(401)") || message.contains("unauthorized")
}

fn make_entry(
    label: Option<String>,
    tokens_result: Option<&Result<Tokens, String>>,
    index_entry: Option<&ProfileIndexEntry>,
    profile_path: &Path,
    ctx: &ListCtx,
    is_current: bool,
) -> Entry {
    let use_color = ctx.use_color;
    let profile_id = profile_id_from_path(profile_path);
    let managed_files = profile_id
        .as_deref()
        .map(|id| managed_files_for_profile(&ctx.profiles_dir, id, index_entry))
        .unwrap_or_else(|| managed_files_for_save(false));
    let label_for_error = label.clone().or_else(|| profile_id.clone());
    let mut tokens = match tokens_result {
        Some(Ok(tokens)) => tokens.clone(),
        Some(Err(err)) => {
            return make_error(
                profile_id,
                label_for_error,
                index_entry,
                managed_files,
                use_color,
                err,
                is_current,
            );
        }
        None => {
            return make_error(
                profile_id,
                label_for_error,
                index_entry,
                managed_files,
                use_color,
                PROFILE_SUMMARY_FILE_MISSING,
                is_current,
            );
        }
    };
    let label_value = label.clone();
    let info = profile_info(Some(&tokens), label, is_current, use_color);
    let is_api_key = is_api_key_profile(&tokens);
    let (details, summary, usage, _) = detail_lines(
        &mut tokens,
        info.email.as_deref(),
        info.plan.as_deref(),
        ctx,
        profile_path,
    );
    Entry {
        id: profile_id,
        label: label_value,
        email: info.email,
        plan: info.plan,
        is_api_key,
        is_saved: true,
        managed_files,
        display: info.display,
        details,
        warnings: Vec::new(),
        usage,
        error_summary: summary,
        always_show_details: info.is_free,
        is_current,
    }
}

fn make_saved(
    id: &str,
    snapshot: &Snapshot,
    labels_by_id: &BTreeMap<String, String>,
    current_saved_id: Option<&str>,
    ctx: &ListCtx,
) -> Entry {
    let profile_path = profile_path_for_id(&ctx.profiles_dir, id);
    let label = labels_by_id.get(id).cloned();
    let is_current = current_saved_id == Some(id);
    make_entry(
        label,
        snapshot.tokens.get(id),
        snapshot.index.profiles.get(id),
        &profile_path,
        ctx,
        is_current,
    )
}

fn make_entries(
    ordered: &[String],
    snapshot: &Snapshot,
    current_saved_id: Option<&str>,
    ctx: &ListCtx,
) -> Vec<Entry> {
    let labels_by_id = labels_by_id(&snapshot.labels);
    let build = |id: &String| make_saved(id, snapshot, &labels_by_id, current_saved_id, ctx);
    if ctx.show_usage && ordered.len() >= 3 {
        let workers = usage_concurrency().min(ordered.len());
        if workers <= 1 {
            return ordered.iter().map(build).collect();
        }
        if let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(workers).build() {
            let mut indexed: Vec<(usize, Entry)> = pool.install(|| {
                ordered
                    .par_iter()
                    .enumerate()
                    .map(|(idx, id)| (idx, build(id)))
                    .collect()
            });
            indexed.sort_by_key(|(idx, _)| *idx);
            return indexed.into_iter().map(|(_, entry)| entry).collect();
        }
        return ordered.iter().map(build).collect();
    }

    ordered.iter().map(build).collect()
}

fn usage_concurrency() -> usize {
    env::var(USAGE_CONCURRENCY_ENV)
        .or_else(|_| env::var(LEGACY_USAGE_CONCURRENCY_ENV))
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .map(|value| value.clamp(1, MAX_USAGE_CONCURRENCY))
        .unwrap_or(DEFAULT_USAGE_CONCURRENCY)
}

fn make_current(
    paths: &Paths,
    current_saved_id: Option<&str>,
    labels: &Labels,
    tokens_map: &BTreeMap<String, Result<Tokens, String>>,
    index: &ProfilesIndex,
    ctx: &ListCtx,
) -> Option<Entry> {
    if !paths.auth.is_file() {
        return None;
    }
    let mut tokens = match read_tokens(&paths.auth) {
        Ok(tokens) => tokens,
        Err(err) => {
            return Some(make_error(
                None,
                None,
                None,
                active_managed_files(paths),
                ctx.use_color,
                &err,
                true,
            ));
        }
    };
    let resolved_saved_id = extract_profile_identity(&tokens).and_then(|identity| {
        let candidates = cached_profile_ids(tokens_map, &identity);
        pick_primary(&candidates)
    });
    let effective_saved_id = current_saved_id.or(resolved_saved_id.as_deref());
    let label = effective_saved_id.and_then(|id| label_for_id(labels, id));
    let managed_files = effective_saved_id
        .map(|id| managed_files_for_profile(&ctx.profiles_dir, id, index.profiles.get(id)))
        .unwrap_or_else(|| active_managed_files(paths));
    let use_color = ctx.use_color;
    let label_value = label.clone();
    let info = profile_info(Some(&tokens), label, true, use_color);
    let plan_is_free = info.is_free;
    let is_api_key = is_api_key_profile(&tokens);
    let can_save = is_profile_ready(&tokens);
    let is_unsaved = effective_saved_id.is_none() && can_save;
    let (mut details, mut summary, mut usage, refreshed) = detail_lines(
        &mut tokens,
        info.email.as_deref(),
        info.plan.as_deref(),
        ctx,
        &paths.auth,
    );
    if refreshed && let Some(saved_id) = effective_saved_id {
        let target = profile_path_for_id(&ctx.profiles_dir, saved_id);
        if let Err(err) = sync_profile(paths, &target) {
            details = vec![format_error(&err)];
            summary = Some(error_summary(PROFILE_SUMMARY_ERROR, &err));
            usage = Some(StatusUsageJson::from_message("error", None, &err));
        }
    }

    let warnings = if is_unsaved {
        format_unsaved_warning(false)
    } else {
        Vec::new()
    };

    if is_unsaved {
        if use_color {
            details.extend(format_unsaved_warning(true));
        } else {
            details.extend(warnings.clone());
        }
    }

    Some(Entry {
        id: effective_saved_id.map(str::to_string),
        label: label_value,
        email: info.email,
        plan: info.plan,
        is_api_key,
        is_saved: effective_saved_id.is_some(),
        managed_files,
        display: info.display,
        details,
        warnings,
        usage,
        error_summary: summary,
        always_show_details: is_unsaved || (plan_is_free && !ctx.show_usage),
        is_current: true,
    })
}

fn error_summary(label: &str, message: &str) -> String {
    let (summary, _) = usage_message_parts(message);
    format!("{label}: {summary}")
}

struct ListCtx {
    base_url: Option<String>,
    base_url_error: Option<String>,
    now: DateTime<Local>,
    show_usage: bool,
    show_current_marker: bool,
    show_id: bool,
    use_color: bool,
    profiles_dir: PathBuf,
}

impl ListCtx {
    fn new(paths: &Paths, show_usage: bool, show_current_marker: bool, show_id: bool) -> Self {
        let (base_url, base_url_error) = if show_usage {
            match read_base_url(paths) {
                Ok(url) => (Some(url), None),
                Err(err) => (None, Some(err)),
            }
        } else {
            (None, None)
        };

        Self {
            base_url,
            base_url_error,
            now: Local::now(),
            show_usage,
            show_current_marker,
            show_id,
            use_color: use_color_stdout(),
            profiles_dir: paths.profiles.clone(),
        }
    }
}

#[derive(Clone)]
struct Entry {
    id: Option<String>,
    label: Option<String>,
    email: Option<String>,
    plan: Option<String>,
    is_api_key: bool,
    is_saved: bool,
    managed_files: Vec<String>,
    display: String,
    details: Vec<String>,
    warnings: Vec<String>,
    usage: Option<StatusUsageJson>,
    error_summary: Option<String>,
    always_show_details: bool,
    is_current: bool,
}

#[derive(Serialize)]
struct ListedProfile {
    id: Option<String>,
    label: Option<String>,
    email: Option<String>,
    plan: Option<String>,
    is_current: bool,
    is_saved: bool,
    is_api_key: bool,
    managed_files: Vec<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct ListedProfiles {
    profiles: Vec<ListedProfile>,
}

#[derive(Serialize)]
struct StatusProfileJson {
    id: Option<String>,
    label: Option<String>,
    email: Option<String>,
    plan: Option<String>,
    is_current: bool,
    is_saved: bool,
    is_api_key: bool,
    managed_files: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    warnings: Vec<String>,
    usage: Option<StatusUsageJson>,
    error: Option<StatusErrorJson>,
}

#[derive(Serialize)]
struct StatusErrorJson {
    summary: StatusErrorSummaryJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize)]
struct StatusErrorSummaryJson {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    response: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct AllStatusJson {
    profiles: Vec<StatusProfileJson>,
}

fn print_list_json(entries: &[Entry]) -> Result<(), String> {
    let profiles = entries
        .iter()
        .map(|entry| ListedProfile {
            id: entry.id.clone(),
            label: entry.label.clone(),
            email: entry.email.clone(),
            plan: entry.plan.clone(),
            is_current: entry.is_current,
            is_saved: entry.is_saved,
            is_api_key: entry.is_api_key,
            managed_files: entry.managed_files.clone(),
            error: entry.error_summary.clone(),
        })
        .collect();
    let json = serde_json::to_string_pretty(&ListedProfiles { profiles })
        .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?;
    println!("{json}");
    Ok(())
}

fn status_error_summary_json(summary: String) -> StatusErrorSummaryJson {
    let summary = crate::sanitize_for_terminal(&summary);
    let Some((start, end, response)) = extract_embedded_json_object(&summary) else {
        return StatusErrorSummaryJson {
            message: summary,
            response: None,
        };
    };

    StatusErrorSummaryJson {
        message: strip_embedded_json_segment(&summary, start, end),
        response: Some(response),
    }
}

fn extract_embedded_json_object(summary: &str) -> Option<(usize, usize, serde_json::Value)> {
    for (start, ch) in summary.char_indices() {
        if ch != '{' {
            continue;
        }
        let Some(end) = find_json_object_end(summary, start) else {
            continue;
        };
        let candidate = &summary[start..end];
        let Ok(value) = serde_json::from_str::<serde_json::Value>(candidate) else {
            continue;
        };
        return Some((start, end, value));
    }
    None
}

fn find_json_object_end(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in text[start..].char_indices() {
        let idx = start + offset;
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }

    None
}

fn strip_embedded_json_segment(text: &str, start: usize, end: usize) -> String {
    let left = text[..start].trim_end_matches([' ', ':']);
    let right = text[end..].trim_start_matches([',', ' ']);
    match (left.is_empty(), right.is_empty()) {
        (true, true) => String::new(),
        (true, false) => right.to_string(),
        (false, true) => left.to_string(),
        (false, false) => format!("{left}, {right}"),
    }
}

fn status_profile_json(entry: Entry) -> StatusProfileJson {
    let mut usage = entry.usage.map(|usage| StatusUsageJson {
        state: usage.state,
        buckets: usage.buckets,
        status_code: usage.status_code,
        summary: usage
            .summary
            .map(|summary| crate::sanitize_for_terminal(&summary)),
        detail: usage
            .detail
            .map(|detail| crate::sanitize_for_terminal(&detail)),
    });
    let mut top_level_summary = entry
        .error_summary
        .map(|error| crate::sanitize_for_terminal(&error));
    let mut error = None;
    if let Some(usage_json) = usage.as_mut()
        && usage_json.state == "error"
    {
        let status_code = usage_json.status_code.take();
        let detail = usage_json.detail.take();
        let usage_summary = usage_json.summary.take();
        let summary = top_level_summary.take().or(usage_summary);
        error = summary.map(|summary| StatusErrorJson {
            summary: status_error_summary_json(summary),
            status_code,
            detail,
        });
    }
    if error.is_none() {
        error = top_level_summary.map(|summary| StatusErrorJson {
            summary: status_error_summary_json(summary),
            status_code: None,
            detail: None,
        });
    }

    StatusProfileJson {
        id: entry.id,
        label: entry.label,
        email: entry.email,
        plan: entry.plan,
        is_current: entry.is_current,
        is_saved: entry.is_saved,
        is_api_key: entry.is_api_key,
        managed_files: entry.managed_files,
        warnings: entry
            .warnings
            .into_iter()
            .map(|warning| crate::sanitize_for_terminal(&warning))
            .collect(),
        usage,
        error,
    }
}

fn print_current_status_json(current: Option<Entry>) -> Result<(), String> {
    let payload = current.map(status_profile_json);
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?;
    println!("{json}");
    Ok(())
}

fn print_all_status_json(profiles: Vec<Entry>) -> Result<(), String> {
    let payload = AllStatusJson {
        profiles: profiles.into_iter().map(status_profile_json).collect(),
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| crate::msg1(PROFILE_ERR_SERIALIZE_INDEX, err))?;
    println!("{json}");
    Ok(())
}

fn handle_inquire_result<T>(
    result: Result<T, inquire::error::InquireError>,
    context: &str,
) -> Result<T, String> {
    match result {
        Ok(value) => Ok(value),
        Err(err) if is_inquire_cancel(&err) => Err(CANCELLED_MESSAGE.to_string()),
        Err(err) => Err(crate::msg2(PROFILE_ERR_PROMPT_CONTEXT, context, err)),
    }
}

fn trim_label(label: &str) -> Result<&str, String> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(PROFILE_ERR_LABEL_EMPTY.to_string());
    }
    Ok(trimmed)
}

#[cfg(test)]
#[path = "profiles/tests.rs"]
mod tests;
