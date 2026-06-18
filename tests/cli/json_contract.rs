use super::*;

#[test]
fn json_save_returns_success_shape() {
    let env = TestEnv::new();
    seed_current(&env);

    let raw = env.run(&["save", "--label", "work", "--json"]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "save");
    assert!(profile["id"].is_string(), "profile.id is string");
    assert_eq!(profile["label"], "work", "profile.label");
    assert_managed_files(profile, &["auth.json"]);
    assert!(profile.get("default").is_none(), "profile.default removed");
}

#[test]
fn json_load_returns_success_shape() {
    let env = TestEnv::new();
    seed_alpha(&env);
    env.run(&["save", "--label", "alpha"]);

    let raw = env.run(&["load", "--label", "alpha", "--json"]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "load");
    assert!(profile["id"].is_string());
    assert_eq!(profile["label"], "alpha");
    assert_managed_files(profile, &["auth.json"]);
    assert!(profile.get("default").is_none());
}

#[test]
fn json_delete_returns_success_shape() {
    let env = TestEnv::new();
    seed_alpha(&env);
    env.run(&["save", "--label", "alpha"]);

    let raw = env.run(&["delete", "--label", "alpha", "--yes", "--json"]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "delete");
    assert!(profile["count"].is_number(), "profile.count is number");
    let deleted = profile["deleted"].as_array().expect("deleted is array");
    assert!(!deleted.is_empty(), "at least one profile deleted");
    assert!(deleted[0]["id"].is_string(), "deleted[0].id is string");
}

#[test]
fn json_label_set_returns_success_shape() {
    let env = TestEnv::new();
    seed_alpha(&env);
    env.run(&["save", "--label", "alpha"]);
    let alpha_id = profile_id_by_label(&env, "alpha");

    let raw = env.run(&[
        "label", "set", "--id", &alpha_id, "--to", "newalpha", "--json",
    ]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "label set");
    assert!(profile["id"].is_string());
    assert_eq!(profile["label"], "newalpha");
    assert!(profile.get("default").is_none());
}

#[test]
fn json_label_clear_returns_success_shape() {
    let env = TestEnv::new();
    seed_alpha(&env);
    env.run(&["save", "--label", "alpha"]);
    let alpha_id = profile_id_by_label(&env, "alpha");

    let raw = env.run(&["label", "clear", "--id", &alpha_id, "--json"]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "label clear");
    assert!(profile["id"].is_string());
    assert!(profile["label"].is_null() || profile["label"] == "");
    assert!(profile.get("default").is_none());
}

#[test]
fn json_label_rename_returns_success_shape() {
    let env = TestEnv::new();
    seed_alpha(&env);
    env.run(&["save", "--label", "alpha"]);

    let raw = env.run(&[
        "label", "rename", "--label", "alpha", "--to", "renamed", "--json",
    ]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "label rename");
    assert!(profile["id"].is_string());
    assert_eq!(profile["label"], "renamed");
    assert!(profile.get("default").is_none());
}

#[test]
fn json_export_returns_success_shape() {
    let env = TestEnv::new();
    seed_alpha(&env);
    seed_beta(&env);
    let out_path = env.home_path().join("exported.json");

    let raw = env.run(&["export", "--output", out_path.to_str().unwrap(), "--json"]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "export");
    assert!(profile["count"].is_number(), "profile.count is number");
    assert!(profile["path"].is_string(), "profile.path is string");
    assert!(out_path.exists(), "export file should exist on disk");
}

#[test]
fn json_import_returns_success_shape() {
    let export_env = TestEnv::new();
    seed_alpha(&export_env);
    seed_beta(&export_env);
    let bundle = export_env.home_path().join("bundle.json");
    export_env.run(&["export", "--output", bundle.to_str().unwrap()]);

    let import_env = TestEnv::new();
    let raw = import_env.run(&["import", "--input", bundle.to_str().unwrap(), "--json"]);
    let v = parse_json(&raw);

    let profile = assert_json_success(&v, "import");
    assert!(profile["count"].is_number());
    assert!(profile["profiles"].is_array());
    let imported = profile["profiles"].as_array().expect("profiles array");
    assert!(imported.iter().all(|entry| entry.get("default").is_none()));
}

#[test]
fn json_mutating_command_error_uses_failure_envelope_on_stderr() {
    let env = TestEnv::new();
    seed_alpha(&env);
    env.run(&["save", "--label", "alpha"]);

    let output = std::process::Command::new(&env.bin_path)
        .args(["delete", "--label", "nonexistent", "--yes", "--json"])
        .env("HOME", env.home_path())
        .env("CODEXSWITCH_CLI_HOME", env.home_path())
        .env("CODEXSWITCH_CLI_COMMAND", "codexswitch-cli")
        .env("CODEXSWITCH_CLI_SKIP_UPDATE", "1")
        .env("NO_COLOR", "1")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to spawn binary");

    assert!(
        !output.status.success(),
        "expected non-zero exit for missing label"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().is_empty(), "stdout should be empty on error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value = parse_json(&stderr);
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["command"], "delete");
    assert_eq!(value["success"], false);
    assert!(value["data"].is_null());
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("nonexistent"))
    );
}
