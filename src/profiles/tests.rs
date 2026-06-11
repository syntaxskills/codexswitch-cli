use super::*;
use crate::test_utils::{build_id_token, make_paths, set_env_guard};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::fs;
use std::path::{Path, PathBuf};

fn write_auth(path: &Path, account_id: &str, email: &str, plan: &str, access: &str, refresh: &str) {
    let id_token = build_id_token(email, plan);
    let value = serde_json::json!({
        "tokens": {
            "account_id": account_id,
            "id_token": id_token,
            "access_token": access,
            "refresh_token": refresh
        }
    });
    fs::write(path, serde_json::to_string(&value).unwrap()).unwrap();
}

fn write_profile(paths: &Paths, id: &str, account_id: &str, email: &str, plan: &str) {
    let id_token = build_id_token(email, plan);
    let value = serde_json::json!({
        "tokens": {
            "account_id": account_id,
            "id_token": id_token,
            "access_token": "acc",
            "refresh_token": "ref"
        }
    });
    let path = profile_path_for_id(&paths.profiles, id);
    fs::create_dir_all(path.parent().expect("profile parent")).unwrap();
    fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
}

fn write_raw_profile(paths: &Paths, id: &str, contents: &str) -> PathBuf {
    let path = profile_path_for_id(&paths.profiles, id);
    fs::create_dir_all(path.parent().expect("profile parent")).unwrap();
    fs::write(&path, contents).unwrap();
    path
}

fn is_snowflake_id(id: &str) -> bool {
    id.len() >= 10 && id.bytes().all(|byte| byte.is_ascii_digit())
}

fn build_id_token_with_user(email: &str, plan: &str, user_id: &str) -> String {
    let header = serde_json::json!({
        "alg": "none",
        "typ": "JWT",
    });
    let auth = serde_json::json!({
        "chatgpt_plan_type": plan,
        "chatgpt_user_id": user_id,
    });
    let payload = serde_json::json!({
        "email": email,
        "https://api.openai.com/auth": auth,
    });
    let header = URL_SAFE_NO_PAD.encode(serde_json::to_string(&header).unwrap());
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_string(&payload).unwrap());
    format!("{header}.{payload}.")
}

fn write_auth_with_user(
    path: &Path,
    account_id: &str,
    email: &str,
    plan: &str,
    user_id: &str,
    access: &str,
    refresh: &str,
) {
    let id_token = build_id_token_with_user(email, plan, user_id);
    let value = serde_json::json!({
        "tokens": {
            "account_id": account_id,
            "id_token": id_token,
            "access_token": access,
            "refresh_token": refresh
        }
    });
    fs::write(path, serde_json::to_string(&value).unwrap()).unwrap();
}

fn make_tokens(account_id: &str, email: &str, plan: &str) -> Tokens {
    Tokens {
        account_id: Some(account_id.to_string()),
        id_token: Some(build_id_token(email, plan)),
        access_token: Some("acc".to_string()),
        refresh_token: Some("ref".to_string()),
    }
}

#[test]
fn require_tty_with_variants() {
    assert!(require_tty_with(true, "load").is_ok());
    let err = require_tty_with(false, "load").unwrap_err();
    assert!(err.contains("requires a TTY"));
}

#[test]
fn prompt_unsaved_load_with_variants() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    let err =
        prompt_unsaved_load_with(&paths, "reason", false, Ok(PROFILE_PROMPT_CANCEL)).unwrap_err();
    assert!(err.contains("not saved"));
    assert!(matches!(
        prompt_unsaved_load_with(&paths, "reason", true, Ok(PROFILE_PROMPT_SAVE_AND_CONTINUE))
            .unwrap(),
        LoadChoice::SaveAndContinue
    ));
    assert!(matches!(
        prompt_unsaved_load_with(
            &paths,
            "reason",
            true,
            Ok(PROFILE_PROMPT_CONTINUE_WITHOUT_SAVING)
        )
        .unwrap(),
        LoadChoice::ContinueWithoutSaving
    ));
    assert!(matches!(
        prompt_unsaved_load_with(&paths, "reason", true, Ok(PROFILE_PROMPT_CANCEL)).unwrap(),
        LoadChoice::Cancel
    ));
    let err = prompt_unsaved_load_with(
        &paths,
        "reason",
        true,
        Err(inquire::error::InquireError::OperationCanceled),
    )
    .unwrap();
    assert!(matches!(err, LoadChoice::Cancel));
}

#[test]
fn confirm_delete_profiles_with_variants() {
    let err = confirm_delete_profiles_with(false, Ok(true)).unwrap_err();
    assert!(err.contains("requires confirmation"));
    assert!(confirm_delete_profiles_with(true, Ok(true)).unwrap());
    let err =
        confirm_delete_profiles_with(true, Err(inquire::error::InquireError::OperationCanceled))
            .unwrap_err();
    assert_eq!(err, CANCELLED_MESSAGE);
}

#[test]
fn label_helpers() {
    let mut labels = Labels::new();
    assign_label(&mut labels, "Team", "id").unwrap();
    assert_eq!(label_for_id(&labels, "id").unwrap(), "Team");
    assert_eq!(resolve_label_id(&labels, "Team").unwrap(), "id");
    remove_labels_for_id(&mut labels, "id");
    assert!(labels.is_empty());
    assert!(trim_label(" ").is_err());
}

#[test]
fn ordered_profile_ids_prefers_current_then_label_then_email() {
    let mut labels = Labels::new();
    labels.insert("alpha".to_string(), "id-a".to_string());
    labels.insert("beta".to_string(), "id-b".to_string());
    labels.insert("zeta".to_string(), "id-z".to_string());

    let mut tokens = BTreeMap::new();
    tokens.insert(
        "id-z".to_string(),
        Ok(make_tokens("acct-z", "z@ex.com", "team")),
    );
    tokens.insert(
        "id-a".to_string(),
        Ok(make_tokens("acct-a", "a@ex.com", "team")),
    );
    tokens.insert(
        "id-u1".to_string(),
        Ok(make_tokens("acct-u1", "c@ex.com", "team")),
    );
    tokens.insert(
        "id-u2".to_string(),
        Ok(make_tokens("acct-u2", "b@ex.com", "team")),
    );
    tokens.insert(
        "id-b".to_string(),
        Ok(make_tokens("acct-b", "d@ex.com", "team")),
    );

    let snapshot = Snapshot {
        labels,
        tokens,
        index: ProfilesIndex::default(),
    };
    let ordered = ordered_profile_ids(&snapshot, Some("id-z"));
    assert_eq!(ordered, vec!["id-z", "id-a", "id-b", "id-u2", "id-u1"]);
}

#[test]
fn usage_concurrency_defaults_and_clamps() {
    let _unset = set_env_guard(USAGE_CONCURRENCY_ENV, None);
    assert_eq!(usage_concurrency(), DEFAULT_USAGE_CONCURRENCY);

    let _zero = set_env_guard(USAGE_CONCURRENCY_ENV, Some("0"));
    assert_eq!(usage_concurrency(), DEFAULT_USAGE_CONCURRENCY);

    let _bad = set_env_guard(USAGE_CONCURRENCY_ENV, Some("oops"));
    assert_eq!(usage_concurrency(), DEFAULT_USAGE_CONCURRENCY);

    let _small = set_env_guard(USAGE_CONCURRENCY_ENV, Some("3"));
    assert_eq!(usage_concurrency(), 3);

    let _high = set_env_guard(USAGE_CONCURRENCY_ENV, Some("999"));
    assert_eq!(usage_concurrency(), MAX_USAGE_CONCURRENCY);
}

#[test]
fn profiles_index_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    let mut index = ProfilesIndex::default();
    index.profiles.insert(
        "id".to_string(),
        ProfileIndexEntry {
            account_id: Some("acct".to_string()),
            email: Some("me@example.com".to_string()),
            plan: Some("Team".to_string()),
            label: Some("work".to_string()),
            is_api_key: false,
            principal_id: Some("principal-1".to_string()),
            workspace_or_org_id: Some("workspace-1".to_string()),
            plan_type_key: Some("team".to_string()),
            managed_files: managed_files_for_save(true),
        },
    );
    write_profiles_index(&paths, &index).unwrap();
    let read_back = read_profiles_index(&paths).unwrap();
    let entry = read_back.profiles.get("id").unwrap();
    assert_eq!(entry.account_id.as_deref(), Some("acct"));
    assert_eq!(entry.email.as_deref(), Some("me@example.com"));
    assert_eq!(entry.plan.as_deref(), Some("Team"));
    assert_eq!(entry.label.as_deref(), Some("work"));
    assert!(!entry.is_api_key);
    assert_eq!(entry.principal_id.as_deref(), Some("principal-1"));
    assert_eq!(entry.workspace_or_org_id.as_deref(), Some("workspace-1"));
    assert_eq!(entry.plan_type_key.as_deref(), Some("team"));
    assert_eq!(
        entry.managed_files,
        vec![AUTH_FILE_NAME.to_string(), CONFIG_FILE_NAME.to_string()]
    );
}

#[test]
fn read_profiles_index_does_not_rewrite_when_legacy_strings_only_appear_in_values() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    let raw = serde_json::json!({
        "version": PROFILES_INDEX_VERSION,
        "profiles": {
            "id": {
                "label": "default_profile_id update_cache active_profile_id last_used",
                "is_api_key": false
            }
        }
    })
    .to_string();
    fs::write(&paths.profiles_index, &raw).unwrap();

    let _ = read_profiles_index(&paths).unwrap();
    let after = fs::read_to_string(&paths.profiles_index).unwrap();
    assert_eq!(after, raw);
}

#[test]
fn profiles_index_prunes_missing_profiles() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    let mut index = ProfilesIndex::default();
    index
        .profiles
        .insert("missing".to_string(), ProfileIndexEntry::default());
    prune_profiles_index(&mut index, &paths.profiles).unwrap();
    assert!(index.profiles.is_empty());
}

#[test]
fn snowflake_profile_ids_are_numeric_and_unique() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    let first = next_snowflake_profile_id(&paths.profiles);
    let second = next_snowflake_profile_id(&paths.profiles);
    assert_ne!(first, second);
    assert!(is_snowflake_id(&first));
    assert!(is_snowflake_id(&second));
}

#[test]
fn load_profile_tokens_map_handles_invalid() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    write_profile(&paths, "valid", "acct", "a@b.com", "pro");
    let bad_path = write_raw_profile(&paths, "bad", "not-json");
    let index = serde_json::json!({
        "version": 1,
        "active_profile_id": null,
        "profiles": {
            "bad": {
                "label": "bad",
                "last_used": 1,
                "added_at": 1
            }
        }
    });
    fs::write(
        &paths.profiles_index,
        serde_json::to_string(&index).unwrap(),
    )
    .unwrap();
    let map = load_profile_tokens_map(&paths).unwrap();
    assert!(map.contains_key("valid"));
    let bad = map.get("bad").expect("bad entry retained");
    assert!(bad.is_err());
    assert!(bad_path.is_file());

    let index_contents = fs::read_to_string(&paths.profiles_index).unwrap();
    assert!(index_contents.contains("\"bad\""));
}

#[test]
fn load_profile_tokens_map_ignores_update_cache_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    fs::write(
        &paths.update_cache,
        serde_json::json!({
            "latest_version": "0.1.0",
            "last_checked_at": "2026-01-01T00:00:00Z"
        })
        .to_string(),
    )
    .unwrap();
    let map = load_profile_tokens_map(&paths).unwrap();
    assert!(map.is_empty());
    assert!(paths.update_cache.is_file());
}

#[cfg(unix)]
#[test]
fn load_profile_tokens_map_remove_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    let bad_path = write_raw_profile(&paths, "bad", "not-json");
    let map = load_profile_tokens_map(&paths).unwrap();
    assert!(map.contains_key("bad"));
    assert!(bad_path.is_file());
}

#[test]
fn resolve_save_and_sync_ids() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    write_profile(&paths, "one", "acct", "a@b.com", "pro");
    let tokens = read_tokens(&profile_path_for_id(&paths.profiles, "one")).unwrap();
    let mut index = ProfilesIndex::default();
    let id = resolve_save_id(&paths, &mut index, &tokens).unwrap();
    assert_eq!(id, "one");
    let id = resolve_sync_id(&paths, &mut index, &tokens).unwrap();
    assert_eq!(id.as_deref(), Some("one"));
}

#[test]
fn render_helpers() {
    let entry = Entry {
        id: Some("alpha@example.com-team".to_string()),
        label: Some("alpha".to_string()),
        email: Some("alpha@example.com".to_string()),
        plan: Some("team".to_string()),
        is_api_key: false,
        is_saved: true,
        managed_files: managed_files_for_save(false),
        display: "Display".to_string(),
        details: vec!["detail".to_string()],
        warnings: Vec::new(),
        usage: None,
        error_summary: None,
        always_show_details: true,
        is_current: false,
    };
    let ctx = ListCtx {
        base_url: None,
        base_url_error: None,
        now: chrono::Local::now(),
        show_usage: false,
        show_current_marker: false,
        show_id: true,
        use_color: false,
        profiles_dir: PathBuf::new(),
    };
    let lines = render_entries(&[entry], &ctx, true);
    assert!(!lines.is_empty());
    push_separator(&mut vec!["a".to_string()], true);
}

#[test]
fn render_entries_preserves_ansi_display_in_color_mode() {
    colored::control::set_override(true);
    let entry = Entry {
        id: Some("alpha@example.com-team".to_string()),
        label: Some("alpha".to_string()),
        email: Some("alpha@example.com".to_string()),
        plan: Some("team".to_string()),
        is_api_key: false,
        is_saved: true,
        managed_files: managed_files_for_save(false),
        display: "\u{1b}[32malpha@example.com\u{1b}[0m".to_string(),
        details: Vec::new(),
        warnings: Vec::new(),
        usage: None,
        error_summary: None,
        always_show_details: false,
        is_current: false,
    };
    let ctx = ListCtx {
        base_url: None,
        base_url_error: None,
        now: chrono::Local::now(),
        show_usage: false,
        show_current_marker: false,
        show_id: false,
        use_color: true,
        profiles_dir: PathBuf::new(),
    };
    let lines = render_entries(&[entry], &ctx, true);
    colored::control::unset_override();

    assert!(!lines.is_empty());
    assert!(lines[0].contains("\u{1b}[32m"));
    assert_eq!(
        crate::ui::strip_ansi(&lines[0]),
        "alpha@example.com [files: auth.json]"
    );
}

#[test]
fn plain_error_lines_merges_unexpected_status_into_summary() {
    let lines = plain_error_lines(
        "deactivated_workspace\nunexpected status 402 Payment Required\nURL: http://localhost/backend-api/wham/usage",
        false,
    );

    assert_eq!(
        lines[0],
        "Error: deactivated_workspace (unexpected status 402 Payment Required)"
    );
    assert_eq!(lines[1], " URL: http://localhost/backend-api/wham/usage");
}

#[test]
fn render_entries_status_all_has_extra_gap_between_profiles() {
    let entries = vec![
        Entry {
            id: Some("one".to_string()),
            label: None,
            email: Some("one@example.com".to_string()),
            plan: Some("team".to_string()),
            is_api_key: false,
            is_saved: true,
            managed_files: managed_files_for_save(false),
            display: "One".to_string(),
            details: vec!["5 hour: 10% left".to_string()],
            warnings: Vec::new(),
            usage: None,
            error_summary: None,
            always_show_details: true,
            is_current: false,
        },
        Entry {
            id: Some("two".to_string()),
            label: None,
            email: Some("two@example.com".to_string()),
            plan: Some("team".to_string()),
            is_api_key: false,
            is_saved: true,
            managed_files: managed_files_for_save(false),
            display: "Two".to_string(),
            details: vec!["5 hour: 20% left".to_string()],
            warnings: Vec::new(),
            usage: None,
            error_summary: None,
            always_show_details: true,
            is_current: false,
        },
    ];
    let ctx = ListCtx {
        base_url: None,
        base_url_error: None,
        now: chrono::Local::now(),
        show_usage: true,
        show_current_marker: false,
        show_id: false,
        use_color: false,
        profiles_dir: PathBuf::new(),
    };
    let lines = render_entries(&entries, &ctx, true);
    let first_profile_last_line = 2;
    assert_eq!(lines[first_profile_last_line + 1], "");
    assert_eq!(lines[first_profile_last_line + 2], "");
}

#[test]
fn strip_ansi_sequences_removes_color_codes() {
    assert_eq!(crate::ui::strip_ansi("\u{1b}[31mtext\u{1b}[0m"), "text");
}

#[test]
fn handle_inquire_result_variants() {
    let ok: Result<i32, inquire::error::InquireError> = Ok(1);
    assert_eq!(handle_inquire_result(ok, "selection").unwrap(), 1);
    let err: Result<(), inquire::error::InquireError> =
        Err(inquire::error::InquireError::OperationCanceled);
    let err = handle_inquire_result(err, "selection").unwrap_err();
    assert_eq!(err, CANCELLED_MESSAGE);
}

#[test]
fn is_http_401_message_variants() {
    assert!(is_http_401_message(&crate::msg2(
        crate::UI_ERROR_TWO_LINE,
        crate::AUTH_REFRESH_401_TITLE,
        crate::AUTH_RELOGIN_AND_SAVE
    )));
    assert!(is_http_401_message("Error: Unauthorized (401)"));
    assert!(!is_http_401_message(&crate::msg1(
        "Error: {}",
        crate::USAGE_UNAVAILABLE_402_TITLE
    )));
}

#[test]
fn sync_and_status_paths() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    write_auth(&paths.auth, "acct", "a@b.com", "pro", "acc", "ref");
    crate::ensure_paths(&paths).unwrap();
    save_profile(&paths, Some("team".to_string()), false, false).unwrap();
    list_profiles(&paths, false, false).unwrap();
    status_profiles(&paths, false, None, None, false).unwrap();
    status_profiles(&paths, true, None, None, false).unwrap();
}

#[test]
fn delete_profile_by_label() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    write_auth(&paths.auth, "acct", "a@b.com", "pro", "acc", "ref");
    crate::ensure_paths(&paths).unwrap();
    save_profile(&paths, Some("team".to_string()), false, false).unwrap();
    delete_profile(&paths, true, Some("team".to_string()), vec![], false).unwrap();
}

#[test]
fn composite_identity_repeated_save_dedupes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    write_auth_with_user(
        &paths.auth,
        "acct-1",
        "same@example.com",
        "pro",
        "user-1",
        "acc",
        "ref",
    );
    crate::ensure_paths(&paths).unwrap();

    save_profile(&paths, None, false, false).unwrap();
    save_profile(&paths, None, false, false).unwrap();

    let ids = collect_profile_ids(&paths.profiles).unwrap();
    assert_eq!(ids.len(), 1);
    assert!(ids.iter().all(|id| is_snowflake_id(id)));
}

#[test]
fn composite_identity_keeps_team_and_pro_separate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    crate::ensure_paths(&paths).unwrap();

    write_auth_with_user(
        &paths.auth,
        "acct-1",
        "same@example.com",
        "pro",
        "user-1",
        "acc",
        "ref",
    );
    save_profile(&paths, None, false, false).unwrap();

    write_auth_with_user(
        &paths.auth,
        "acct-1",
        "same@example.com",
        "team",
        "user-1",
        "acc",
        "ref",
    );
    save_profile(&paths, None, false, false).unwrap();

    let ids = collect_profile_ids(&paths.profiles).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| is_snowflake_id(id)));
}

#[test]
fn composite_identity_separates_users_in_same_workspace_plan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = make_paths(dir.path());
    fs::create_dir_all(&paths.profiles).unwrap();
    crate::ensure_paths(&paths).unwrap();

    write_auth_with_user(
        &paths.auth,
        "acct-1",
        "same@example.com",
        "pro",
        "user-1",
        "acc",
        "ref",
    );
    save_profile(&paths, None, false, false).unwrap();

    write_auth_with_user(
        &paths.auth,
        "acct-1",
        "same@example.com",
        "pro",
        "user-2",
        "acc",
        "ref",
    );
    save_profile(&paths, None, false, false).unwrap();

    let ids = collect_profile_ids(&paths.profiles).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| is_snowflake_id(id)));
}
