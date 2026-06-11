use std::collections::BTreeMap;

use super::{Labels, ProfileStore, trim_label};
use crate::{
    PROFILE_ERR_ID_NO_MATCH, PROFILE_ERR_LABEL_EXISTS, PROFILE_ERR_LABEL_NOT_FOUND,
    format_list_hint, use_color_stderr,
};

pub(crate) fn prune_labels(labels: &mut Labels, profiles_dir: &std::path::Path) {
    labels.retain(|_, id| super::profile_path_for_id(profiles_dir, id).is_file());
}

pub(crate) fn assign_label(labels: &mut Labels, label: &str, id: &str) -> Result<(), String> {
    let trimmed = trim_label(label)?;
    if let Some(existing) = labels.get(trimmed)
        && existing != id
    {
        return Err(crate::msg2(
            PROFILE_ERR_LABEL_EXISTS,
            trimmed,
            format_list_hint(use_color_stderr()),
        ));
    }
    remove_labels_for_id(labels, id);
    labels.insert(trimmed.to_string(), id.to_string());
    Ok(())
}

pub(crate) fn remove_labels_for_id(labels: &mut Labels, id: &str) {
    labels.retain(|_, value| value != id);
}

pub(crate) fn label_for_id(labels: &Labels, id: &str) -> Option<String> {
    labels.iter().find_map(|(label, value)| {
        if value == id {
            Some(label.clone())
        } else {
            None
        }
    })
}

pub(super) fn labels_by_id(labels: &Labels) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (label, id) in labels {
        out.entry(id.clone()).or_insert_with(|| label.clone());
    }
    out
}

pub(crate) fn resolve_label_id(labels: &Labels, label: &str) -> Result<String, String> {
    let trimmed = trim_label(label)?;
    labels.get(trimmed).cloned().ok_or_else(|| {
        crate::msg2(
            PROFILE_ERR_LABEL_NOT_FOUND,
            trimmed,
            format_list_hint(use_color_stderr()),
        )
    })
}

pub(super) fn resolve_label_target_id(
    store: &ProfileStore,
    label: Option<&str>,
    id: Option<&str>,
) -> Result<String, String> {
    if let Some(label) = label {
        return resolve_label_id(&store.labels, label);
    }

    let Some(id) = id else {
        unreachable!("clap enforces label target selector")
    };
    if store.profiles_index.profiles.contains_key(id) {
        return Ok(id.to_string());
    }
    Err(crate::msg2(
        PROFILE_ERR_ID_NO_MATCH,
        id,
        format_list_hint(use_color_stderr()),
    ))
}
