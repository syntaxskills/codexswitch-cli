use std::fs;
use std::path::Path;

use toml_edit::DocumentMut;

use crate::{Paths, common::write_atomic_private};

const MANAGED_CONFIG_KEYS_FIELD: &str = "managed_config_keys";

pub(crate) fn write_config_snapshot(
    paths: &Paths,
    source: &Path,
    target: &Path,
) -> Result<(), String> {
    let Some(managed_keys) = managed_config_keys(paths)? else {
        return copy_config(source, target);
    };
    let snapshot = selected_snapshot(&read_document(source)?, &managed_keys);
    write_document(target, &snapshot)
}

pub(crate) fn apply_config_snapshot(
    paths: &Paths,
    snapshot: &Path,
    target: &Path,
) -> Result<(), String> {
    let Some(managed_keys) = managed_config_keys(paths)? else {
        return copy_config(snapshot, target);
    };
    let snapshot = selected_snapshot(&read_document(snapshot)?, &managed_keys);
    let mut target_document = if target.is_file() {
        read_document(target)?
    } else {
        DocumentMut::new()
    };

    for key in &managed_keys {
        target_document.remove(key);
    }
    for key in &managed_keys {
        if let Some(item) = snapshot.get(key) {
            target_document.insert(key, item.clone());
        }
    }

    write_document(target, &target_document)
}

fn managed_config_keys(paths: &Paths) -> Result<Option<Vec<String>>, String> {
    let config_path = paths.codex.join("codexswitch").join("config.toml");
    if !config_path.is_file() {
        return Ok(None);
    }

    let config = read_document(&config_path)?;
    let Some(item) = config.get(MANAGED_CONFIG_KEYS_FIELD) else {
        return Ok(None);
    };
    normalize_keys(
        parse_key_array(item, MANAGED_CONFIG_KEYS_FIELD, &config_path)?,
        &config_path,
    )
    .map(Some)
}

fn parse_key_array(
    item: &toml_edit::Item,
    field: &str,
    config_path: &Path,
) -> Result<Vec<String>, String> {
    let Some(array) = item.as_array() else {
        return Err(format!(
            "CodexSwitch config {} field '{}' must be an array of top-level config keys.",
            config_path.display(),
            field
        ));
    };
    array
        .iter()
        .map(|value| {
            value.as_str().map(str::to_string).ok_or_else(|| {
                format!(
                    "CodexSwitch config {} field '{}' must contain only strings.",
                    config_path.display(),
                    field
                )
            })
        })
        .collect()
}

fn normalize_keys(keys: Vec<String>, config_path: &Path) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for key in keys {
        let key = key.trim();
        if key.is_empty() || key.contains('.') {
            return Err(format!(
                "CodexSwitch config {} managed config key '{}' must be a non-empty top-level key.",
                config_path.display(),
                key
            ));
        }
        if !normalized.iter().any(|existing| existing == key) {
            normalized.push(key.to_string());
        }
    }
    Ok(normalized)
}

fn selected_snapshot(document: &DocumentMut, managed_keys: &[String]) -> DocumentMut {
    let mut snapshot = DocumentMut::new();
    for key in managed_keys {
        if let Some(item) = document.get(key) {
            snapshot.insert(key, item.clone());
        }
    }
    snapshot
}

fn read_document(path: &Path) -> Result<DocumentMut, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("Could not read config {}: {err}", path.display()))?;
    contents
        .parse::<DocumentMut>()
        .map_err(|err| format!("Could not parse config {}: {err}", path.display()))
}

fn write_document(path: &Path, document: &DocumentMut) -> Result<(), String> {
    let mut contents = document.to_string().into_bytes();
    if !contents.ends_with(b"\n") {
        contents.push(b'\n');
    }
    write_atomic_private(path, &contents)
        .map_err(|err| format!("Could not write config {}: {err}", path.display()))
}

fn copy_config(source: &Path, target: &Path) -> Result<(), String> {
    let contents = fs::read(source)
        .map_err(|err| format!("Could not read config {}: {err}", source.display()))?;
    write_atomic_private(target, &contents)
        .map_err(|err| format!("Could not write config {}: {err}", target.display()))
}

#[cfg(test)]
mod tests {
    use super::{apply_config_snapshot, write_config_snapshot};
    use crate::test_utils::make_paths;
    use std::fs;

    #[test]
    fn default_snapshot_copies_entire_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = make_paths(dir.path());
        let source = dir.path().join("source.toml");
        let snapshot = dir.path().join("snapshot.toml");
        fs::write(
            &source,
            r#"
model = "custom-model"
model_provider = "custom"
approval_policy = "never"

[model_providers.custom]
base_url = "https://provider.example/v1"

[mcp_servers.keep]
command = "server"
"#,
        )
        .expect("write source");

        write_config_snapshot(&paths, &source, &snapshot).expect("write snapshot");
        let contents = fs::read_to_string(&snapshot).expect("read snapshot");

        assert!(contents.contains("model = \"custom-model\""));
        assert!(contents.contains("[model_providers.custom]"));
        assert!(contents.contains("approval_policy"));
        assert!(contents.contains("mcp_servers"));
    }

    #[test]
    fn default_apply_replaces_entire_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = make_paths(dir.path());
        let snapshot = dir.path().join("snapshot.toml");
        let target = dir.path().join("target.toml");
        fs::write(
            &snapshot,
            r#"
model_provider = "custom"

[model_providers.custom]
base_url = "https://provider.example/v1"
"#,
        )
        .expect("write snapshot");
        fs::write(
            &target,
            r#"
model_provider = "old"
approval_policy = "on-request"

[model_providers.old]
base_url = "https://old.example/v1"

[mcp_servers.keep]
command = "server"
"#,
        )
        .expect("write target");

        apply_config_snapshot(&paths, &snapshot, &target).expect("apply snapshot");
        let contents = fs::read_to_string(target).expect("read target");

        assert!(contents.contains("model_provider = \"custom\""));
        assert!(contents.contains("[model_providers.custom]"));
        assert!(!contents.contains("old.example"));
        assert!(!contents.contains("approval_policy"));
        assert!(!contents.contains("[mcp_servers.keep]"));
    }

    #[test]
    fn configured_keys_are_saved_and_replaced_selectively() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = make_paths(dir.path());
        let settings_dir = paths.codex.join("codexswitch");
        fs::create_dir_all(&settings_dir).expect("create settings dir");
        fs::write(
            settings_dir.join("config.toml"),
            "managed_config_keys = [\"chatgpt_base_url\"]\n",
        )
        .expect("write settings");
        let source = dir.path().join("source.toml");
        let snapshot = dir.path().join("snapshot.toml");
        let target = dir.path().join("target.toml");
        fs::write(
            &source,
            "chatgpt_base_url = \"https://new.example\"\napproval_policy = \"never\"\n",
        )
        .expect("write source");
        fs::write(
            &target,
            "chatgpt_base_url = \"https://old.example\"\napproval_policy = \"on-request\"\n",
        )
        .expect("write target");

        write_config_snapshot(&paths, &source, &snapshot).expect("write snapshot");
        let snapshot_contents = fs::read_to_string(&snapshot).expect("read snapshot");
        assert!(snapshot_contents.contains("https://new.example"));
        assert!(!snapshot_contents.contains("approval_policy"));

        apply_config_snapshot(&paths, &snapshot, &target).expect("apply snapshot");
        let contents = fs::read_to_string(target).expect("read target");

        assert!(contents.contains("https://new.example"));
        assert!(contents.contains("approval_policy = \"on-request\""));
    }

    #[test]
    fn empty_managed_key_list_saves_nothing_and_changes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = make_paths(dir.path());
        let settings_dir = paths.codex.join("codexswitch");
        fs::create_dir_all(&settings_dir).expect("create settings dir");
        fs::write(
            settings_dir.join("config.toml"),
            "managed_config_keys = []\n",
        )
        .expect("write settings");
        let source = dir.path().join("source.toml");
        let snapshot = dir.path().join("snapshot.toml");
        let target = dir.path().join("target.toml");
        fs::write(
            &source,
            "model_provider = \"custom\"\napproval_policy = \"never\"\n",
        )
        .expect("write source");
        fs::write(&target, "approval_policy = \"on-request\"\n").expect("write target");

        write_config_snapshot(&paths, &source, &snapshot).expect("write snapshot");
        let contents = fs::read_to_string(&snapshot).expect("read snapshot");
        assert!(contents.trim().is_empty());

        apply_config_snapshot(&paths, &snapshot, &target).expect("apply snapshot");
        assert_eq!(
            fs::read_to_string(target).expect("read target"),
            "approval_policy = \"on-request\"\n"
        );
    }

    #[test]
    fn settings_without_managed_keys_keep_full_config_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = make_paths(dir.path());
        let settings_dir = paths.codex.join("codexswitch");
        fs::create_dir_all(&settings_dir).expect("create settings dir");
        fs::write(
            settings_dir.join("config.toml"),
            "some_future_setting = true\n",
        )
        .expect("write settings");
        let source = dir.path().join("source.toml");
        let snapshot = dir.path().join("snapshot.toml");
        fs::write(
            &source,
            "model_provider = \"custom\"\napproval_policy = \"never\"\n",
        )
        .expect("write source");

        write_config_snapshot(&paths, &source, &snapshot).expect("write snapshot");
        let contents = fs::read_to_string(snapshot).expect("read snapshot");

        assert!(contents.contains("model_provider"));
        assert!(contents.contains("approval_policy"));
    }
}
