use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::IsTerminal as _;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration as StdDuration;

use crate::{Paths, lock_usage, read_profiles_index, write_atomic, write_profiles_index};
use crate::{
    UPDATE_ERR_PERSIST_DISMISSAL, UPDATE_ERR_READ_CHOICE, UPDATE_ERR_REFRESH_VERSION,
    UPDATE_ERR_SHOW_PROMPT, UPDATE_NON_TTY_RUN, UPDATE_OPTION_NOW, UPDATE_OPTION_SKIP,
    UPDATE_OPTION_SKIP_VERSION, UPDATE_PROMPT_SELECT, UPDATE_RELEASE_NOTES, UPDATE_TITLE_AVAILABLE,
};

// We use the latest version from the cask if installation is via homebrew - homebrew does not immediately pick up the latest release and can lag behind.
const HOMEBREW_CASK_URL: &str =
    "https://raw.githubusercontent.com/Homebrew/homebrew-cask/HEAD/Casks/c/codexswitch-cli.rb";
const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/syntaxskills/codexswitch-cli/releases/latest";
const RELEASE_NOTES_URL: &str = "https://github.com/syntaxskills/codexswitch-cli/releases/latest";
#[cfg(test)]
const HOMEBREW_CASK_URL_OVERRIDE_ENV_VAR: &str = "CODEX_PROFILES_HOMEBREW_CASK_URL";
#[cfg(test)]
const LATEST_RELEASE_URL_OVERRIDE_ENV_VAR: &str = "CODEX_PROFILES_LATEST_RELEASE_URL";

/// Update action the CLI should perform after the prompt exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `npm install -g codexswitch-cli`.
    NpmGlobalLatest,
    /// Update via `bun install -g codexswitch-cli`.
    BunGlobalLatest,
    /// Update via `brew upgrade codexswitch-cli`.
    BrewUpgrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallSource {
    Npm,
    Bun,
    Brew,
    Unknown,
}

impl UpdateAction {
    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "codexswitch-cli"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "codexswitch-cli"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "codexswitch-cli"]),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

pub fn detect_install_source() -> InstallSource {
    let exe = std::env::current_exe().unwrap_or_default();
    let managed_by_npm = std::env::var_os("CODEXSWITCH_CLI_MANAGED_BY_NPM")
        .or_else(|| std::env::var_os("CODEX_PROFILES_MANAGED_BY_NPM"))
        .is_some();
    let managed_by_bun = std::env::var_os("CODEXSWITCH_CLI_MANAGED_BY_BUN")
        .or_else(|| std::env::var_os("CODEX_PROFILES_MANAGED_BY_BUN"))
        .is_some();
    detect_install_source_inner(
        cfg!(target_os = "macos"),
        &exe,
        managed_by_npm,
        managed_by_bun,
    )
}

#[doc(hidden)]
pub fn detect_install_source_inner(
    is_macos: bool,
    current_exe: &std::path::Path,
    managed_by_npm: bool,
    managed_by_bun: bool,
) -> InstallSource {
    let bun_install = is_bun_install(current_exe);
    if managed_by_npm {
        InstallSource::Npm
    } else if managed_by_bun && bun_install {
        InstallSource::Bun
    } else if is_macos && is_brew_install(current_exe) {
        InstallSource::Brew
    } else if bun_install {
        InstallSource::Bun
    } else if managed_by_bun {
        // Defensive fallback: older wrappers may leak bun hints from ambient env
        // even when the installed binary is not managed by bun.
        InstallSource::Npm
    } else {
        InstallSource::Unknown
    }
}

fn is_bun_install(current_exe: &std::path::Path) -> bool {
    let exe = current_exe.to_string_lossy();
    exe.contains(".bun/install/global") || exe.contains(".bun\\install\\global")
}

fn is_brew_install(current_exe: &std::path::Path) -> bool {
    (current_exe.starts_with("/opt/homebrew") || current_exe.starts_with("/usr/local"))
        && current_exe.file_name().and_then(|name| name.to_str()) == Some("codexswitch-cli")
}

pub(crate) fn get_update_action() -> Option<UpdateAction> {
    get_update_action_with_debug(cfg!(debug_assertions), detect_install_source())
}

fn get_update_action_with_debug(
    is_debug: bool,
    install_source: InstallSource,
) -> Option<UpdateAction> {
    if is_debug {
        return None;
    }
    match install_source {
        InstallSource::Npm => Some(UpdateAction::NpmGlobalLatest),
        InstallSource::Bun => Some(UpdateAction::BunGlobalLatest),
        InstallSource::Brew => Some(UpdateAction::BrewUpgrade),
        InstallSource::Unknown => None,
    }
}

#[derive(Clone, Debug)]
pub struct UpdateConfig {
    pub codex_home: PathBuf,
    pub check_for_update_on_startup: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct ReleaseInfo {
    tag_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateCache {
    #[serde(default)]
    latest_version: String,
    #[serde(default = "update_cache_checked_default")]
    last_checked_at: DateTime<Utc>,
    #[serde(default)]
    dismissed_version: Option<String>,
    #[serde(default)]
    last_prompted_at: Option<DateTime<Utc>>,
}

fn update_cache_checked_default() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_else(Utc::now)
}

pub enum UpdatePromptOutcome {
    Continue,
    RunUpdate(UpdateAction),
}

pub fn run_update_prompt_if_needed(config: &UpdateConfig) -> Result<UpdatePromptOutcome, String> {
    let mut input = io::stdin().lock();
    let mut output = io::stderr();
    run_update_prompt_if_needed_with_io(
        config,
        cfg!(debug_assertions),
        io::stdin().is_terminal(),
        &mut input,
        &mut output,
    )
}

fn run_update_prompt_if_needed_with_io(
    config: &UpdateConfig,
    is_debug: bool,
    is_tty: bool,
    input: &mut impl io::BufRead,
    output: &mut impl Write,
) -> Result<UpdatePromptOutcome, String> {
    run_update_prompt_if_needed_with_io_and_source(
        config,
        is_debug,
        is_tty,
        detect_install_source(),
        input,
        output,
    )
}

fn run_update_prompt_if_needed_with_io_and_source(
    config: &UpdateConfig,
    is_debug: bool,
    is_tty: bool,
    install_source: InstallSource,
    input: &mut impl io::BufRead,
    output: &mut impl Write,
) -> Result<UpdatePromptOutcome, String> {
    if is_debug {
        return Ok(UpdatePromptOutcome::Continue);
    }

    let Some(latest_version) = get_upgrade_version_for_popup_with_debug(config, is_debug) else {
        return Ok(UpdatePromptOutcome::Continue);
    };
    let Some(update_action) = get_update_action_with_debug(false, install_source) else {
        return Ok(UpdatePromptOutcome::Continue);
    };

    let current_version = current_version();
    if !is_tty {
        write_prompt(
            output,
            format_args!(
                "{} {current_version} -> {latest_version}\n",
                UPDATE_TITLE_AVAILABLE
            ),
        )?;
        write_prompt(
            output,
            format_args!(
                "{}",
                crate::msg1(UPDATE_NON_TTY_RUN, update_action.command_str())
            ),
        )?;
        return Ok(UpdatePromptOutcome::Continue);
    }

    write_prompt(
        output,
        format_args!(
            "\n✨ {} {current_version} -> {latest_version}\n",
            UPDATE_TITLE_AVAILABLE
        ),
    )?;
    write_prompt(
        output,
        format_args!("{}", crate::msg1(UPDATE_RELEASE_NOTES, RELEASE_NOTES_URL)),
    )?;
    write_prompt(output, format_args!("\n"))?;
    write_prompt(
        output,
        format_args!(
            "{}",
            crate::msg1(UPDATE_OPTION_NOW, update_action.command_str())
        ),
    )?;
    write_prompt(output, format_args!("{}", UPDATE_OPTION_SKIP))?;
    write_prompt(output, format_args!("{}", UPDATE_OPTION_SKIP_VERSION))?;
    write_prompt(output, format_args!("{}", UPDATE_PROMPT_SELECT))?;
    output.flush().map_err(prompt_io_error)?;
    let _ = mark_prompted_now(config);

    let mut selection = String::new();
    input
        .read_line(&mut selection)
        .map_err(|err| crate::msg1(UPDATE_ERR_READ_CHOICE, err))?;

    match selection.trim() {
        "1" => Ok(UpdatePromptOutcome::RunUpdate(update_action)),
        "3" => {
            if let Err(err) = dismiss_version(config, &latest_version) {
                write_prompt(
                    output,
                    format_args!("{}", crate::msg1(UPDATE_ERR_PERSIST_DISMISSAL, err)),
                )?;
            }
            Ok(UpdatePromptOutcome::Continue)
        }
        _ => Ok(UpdatePromptOutcome::Continue),
    }
}

fn prompt_io_error(err: impl std::fmt::Display) -> String {
    crate::msg1(UPDATE_ERR_SHOW_PROMPT, err)
}

fn write_prompt(output: &mut impl Write, args: std::fmt::Arguments) -> Result<(), String> {
    output.write_fmt(args).map_err(prompt_io_error)
}

fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn build_update_cache(
    latest_version: Option<String>,
    dismissed_version: Option<String>,
    last_prompted_at: Option<DateTime<Utc>>,
) -> UpdateCache {
    UpdateCache {
        latest_version: latest_version.unwrap_or_else(|| current_version().to_string()),
        last_checked_at: Utc::now(),
        dismissed_version,
        last_prompted_at,
    }
}

fn get_upgrade_version_with_debug(config: &UpdateConfig, is_debug: bool) -> Option<String> {
    if updates_disabled_with_debug(config, is_debug) {
        return None;
    }
    let paths = paths_for_update(config.codex_home.clone());
    let mut info = read_update_cache(&paths).ok().flatten();

    let should_check = match &info {
        None => true,
        Some(info) => info.last_checked_at < Utc::now() - Duration::hours(20),
    };
    if should_check {
        if info.is_none() {
            if let Err(err) = check_for_update(&paths) {
                eprintln!("{}", crate::msg1(UPDATE_ERR_REFRESH_VERSION, err));
            }
            info = read_update_cache(&paths).ok().flatten();
        } else {
            let codex_home = config.codex_home.clone();
            std::thread::spawn(move || {
                let paths = paths_for_update(codex_home);
                if let Err(err) = check_for_update(&paths) {
                    eprintln!("{}", crate::msg1(UPDATE_ERR_REFRESH_VERSION, err));
                }
            });
        }
    }

    info.and_then(|info| {
        if is_newer(&info.latest_version, current_version()).unwrap_or(false) {
            Some(info.latest_version)
        } else {
            None
        }
    })
}

fn check_for_update(paths: &Paths) -> Result<(), String> {
    check_for_update_with_action(paths, get_update_action())
}

fn check_for_update_with_action(
    paths: &Paths,
    update_action: Option<UpdateAction>,
) -> Result<(), String> {
    let latest_version = match update_action {
        Some(UpdateAction::BrewUpgrade) => {
            fetch_version_from_cask().or_else(fetch_version_from_release)
        }
        _ => fetch_version_from_release(),
    };

    // Preserve any previously dismissed version if present.
    let prev_info = read_update_cache(paths).ok().flatten();
    let prev_dismissed = prev_info
        .as_ref()
        .and_then(|info| info.dismissed_version.clone());
    let prev_prompted = prev_info.as_ref().and_then(|info| info.last_prompted_at);
    let info = build_update_cache(latest_version, prev_dismissed, prev_prompted);
    write_update_cache(paths, &info)
}

#[doc(hidden)]
pub fn is_newer(latest: &str, current: &str) -> Option<bool> {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => Some(l > c),
        _ => None,
    }
}

#[doc(hidden)]
pub fn extract_version_from_cask(cask_contents: &str) -> Result<String, String> {
    cask_contents
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("version \"")
                .and_then(|rest| rest.strip_suffix('"'))
                .map(ToString::to_string)
        })
        .ok_or_else(|| "Failed to find version in Homebrew cask file".to_string())
}

#[doc(hidden)]
pub fn extract_version_from_latest_tag(latest_tag_name: &str) -> Result<String, String> {
    for prefix in ["v", "rust-v"] {
        if let Some(version) = latest_tag_name.strip_prefix(prefix) {
            return Ok(version.to_string());
        }
    }
    Err(format!(
        "Failed to parse latest tag name '{latest_tag_name}'"
    ))
}

fn fetch_version_from_cask() -> Option<String> {
    let response = update_agent()
        .get(&homebrew_cask_url())
        .header("User-Agent", "codexswitch-cli")
        .call();
    match response {
        Ok(mut resp) => {
            let contents = resp.body_mut().read_to_string().ok()?;
            extract_version_from_cask(&contents).ok()
        }
        Err(ureq::Error::StatusCode(404)) => None,
        Err(_) => None,
    }
}

fn fetch_version_from_release() -> Option<String> {
    let response = update_agent()
        .get(&latest_release_url())
        .header("User-Agent", "codexswitch-cli")
        .call();
    match response {
        Ok(mut resp) => {
            let ReleaseInfo {
                tag_name: latest_tag_name,
            } = resp.body_mut().read_json().ok()?;
            extract_version_from_latest_tag(&latest_tag_name).ok()
        }
        Err(ureq::Error::StatusCode(404)) => None,
        Err(_) => None,
    }
}

fn get_upgrade_version_for_popup_with_debug(
    config: &UpdateConfig,
    is_debug: bool,
) -> Option<String> {
    if updates_disabled_with_debug(config, is_debug) {
        return None;
    }

    let paths = paths_for_update(config.codex_home.clone());
    let latest = get_upgrade_version_with_debug(config, is_debug)?;
    let info = read_update_cache(&paths).ok().flatten();
    if info
        .as_ref()
        .and_then(|info| info.last_prompted_at)
        .is_some_and(|last| last > Utc::now() - Duration::hours(24))
    {
        return None;
    }
    // If the user dismissed this exact version previously, do not show the popup.
    if info
        .as_ref()
        .and_then(|info| info.dismissed_version.as_deref())
        == Some(latest.as_str())
    {
        return None;
    }
    Some(latest)
}

/// Persist a dismissal for the current latest version so we don't show
/// the update popup again for this version.
pub fn dismiss_version(config: &UpdateConfig, version: &str) -> Result<(), String> {
    if !config.check_for_update_on_startup {
        return Ok(());
    }
    let paths = paths_for_update(config.codex_home.clone());
    let mut info = match read_update_cache(&paths) {
        Ok(Some(info)) => info,
        _ => return Ok(()),
    };
    info.dismissed_version = Some(version.to_string());
    info.last_prompted_at = Some(Utc::now());
    write_update_cache(&paths, &info)
}

fn mark_prompted_now(config: &UpdateConfig) -> Result<(), String> {
    if !config.check_for_update_on_startup {
        return Ok(());
    }
    let paths = paths_for_update(config.codex_home.clone());
    let mut info = match read_update_cache(&paths) {
        Ok(Some(info)) => info,
        _ => return Ok(()),
    };
    info.last_prompted_at = Some(Utc::now());
    write_update_cache(&paths, &info)
}

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut iter = v.trim().split('.');
    let maj = iter.next()?.parse::<u64>().ok()?;
    let min = iter.next()?.parse::<u64>().ok()?;
    let pat = iter.next()?.parse::<u64>().ok()?;
    Some((maj, min, pat))
}

fn updates_disabled_with_debug(config: &UpdateConfig, is_debug: bool) -> bool {
    is_debug || !config.check_for_update_on_startup
}

fn paths_for_update(codex_home: PathBuf) -> Paths {
    let profiles = codex_home.join("profiles");
    Paths {
        auth: codex_home.join("auth.json"),
        config: codex_home.join("config.toml"),
        profiles_index: profiles.join("profiles.json"),
        update_cache: profiles.join("update.json"),
        profiles_lock: profiles.join("profiles.lock"),
        codex: codex_home,
        profiles,
    }
}

fn read_update_cache(paths: &Paths) -> Result<Option<UpdateCache>, String> {
    if !paths.update_cache.is_file() {
        if let Some(legacy) = read_legacy_update_cache(paths)? {
            let _ = write_update_cache(paths, &legacy);
            return Ok(Some(legacy));
        }
        return Ok(None);
    }
    let contents = fs::read_to_string(&paths.update_cache).map_err(|e| e.to_string())?;
    if contents.trim().is_empty() {
        return Ok(None);
    }
    let cache = serde_json::from_str::<UpdateCache>(&contents).map_err(|e| e.to_string())?;
    Ok(Some(cache))
}

fn write_update_cache(paths: &Paths, cache: &UpdateCache) -> Result<(), String> {
    let _lock = lock_usage(paths)?;
    let contents = serde_json::to_string_pretty(cache).map_err(|e| e.to_string())?;
    write_atomic(&paths.update_cache, format!("{contents}\n").as_bytes())
}

fn read_legacy_update_cache(paths: &Paths) -> Result<Option<UpdateCache>, String> {
    if !paths.profiles_index.is_file() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&paths.profiles_index).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&contents).map_err(|e| e.to_string())?;
    let Some(value) = json.get("update_cache") else {
        return Ok(None);
    };
    let cache = serde_json::from_value::<UpdateCache>(value.clone()).map_err(|e| e.to_string())?;
    if let Ok(index) = read_profiles_index(paths) {
        let _ = write_profiles_index(paths, &index);
    }
    Ok(Some(cache))
}

fn update_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(StdDuration::from_secs(5)))
        .build();
    config.into()
}

fn latest_release_url() -> String {
    #[cfg(test)]
    {
        env_url(LATEST_RELEASE_URL_OVERRIDE_ENV_VAR, LATEST_RELEASE_URL)
    }
    #[cfg(not(test))]
    {
        LATEST_RELEASE_URL.to_string()
    }
}

fn homebrew_cask_url() -> String {
    #[cfg(test)]
    {
        env_url(HOMEBREW_CASK_URL_OVERRIDE_ENV_VAR, HOMEBREW_CASK_URL)
    }
    #[cfg(not(test))]
    {
        HOMEBREW_CASK_URL.to_string()
    }
}

#[cfg(test)]
fn env_url(override_var: &str, default: &str) -> String {
    std::env::var(override_var).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{ENV_MUTEX, http_ok_response, set_env_guard, spawn_server};
    use std::fs;
    use std::path::PathBuf;

    fn seed_version_info(config: &UpdateConfig, version: &str) {
        let paths = paths_for_update(config.codex_home.clone());
        fs::create_dir_all(&paths.profiles).unwrap();
        fs::write(&paths.profiles_lock, "").unwrap();
        let info = UpdateCache {
            latest_version: version.to_string(),
            last_checked_at: Utc::now(),
            dismissed_version: None,
            last_prompted_at: None,
        };
        write_update_cache(&paths, &info).unwrap();
    }

    #[test]
    fn update_action_commands() {
        let (cmd, args) = UpdateAction::NpmGlobalLatest.command_args();
        assert_eq!(cmd, "npm");
        assert!(args.contains(&"install"));
        assert!(UpdateAction::BunGlobalLatest.command_str().contains("bun"));
    }

    #[test]
    fn detect_install_source_inner_variants() {
        let exe = PathBuf::from("/usr/local/bin/codexswitch-cli");
        assert_eq!(
            detect_install_source_inner(true, &exe, false, false),
            InstallSource::Brew
        );
        assert_eq!(
            detect_install_source_inner(false, &exe, true, false),
            InstallSource::Npm
        );
        assert_eq!(
            detect_install_source_inner(false, &exe, false, true),
            InstallSource::Npm
        );

        let bun_exe = PathBuf::from(
            "/Users/dev/.bun/install/global/node_modules/codexswitch-cli/bin/codexswitch-cli",
        );
        assert_eq!(
            detect_install_source_inner(false, &bun_exe, false, true),
            InstallSource::Bun
        );
        assert_eq!(
            detect_install_source_inner(false, &bun_exe, false, false),
            InstallSource::Bun
        );
    }

    #[test]
    fn get_update_action_debug() {
        assert!(get_update_action_with_debug(true, InstallSource::Npm).is_none());
        assert!(get_update_action_with_debug(false, InstallSource::Npm).is_some());
    }

    #[test]
    fn extract_version_helpers() {
        assert_eq!(extract_version_from_latest_tag("v1.2.3").unwrap(), "1.2.3");
        assert_eq!(
            extract_version_from_latest_tag("rust-v2.0.0").unwrap(),
            "2.0.0"
        );
        assert!(extract_version_from_latest_tag("bad").is_err());
        let cask = "version \"1.2.3\"";
        assert_eq!(extract_version_from_cask(cask).unwrap(), "1.2.3");
        assert!(extract_version_from_cask("nope").is_err());
    }

    #[test]
    fn parse_version_and_compare() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert!(is_newer("2.0.0", "1.9.9").unwrap());
        assert!(is_newer("bad", "1.0.0").is_none());
    }

    #[test]
    fn url_overrides_work() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let _env = set_env_guard(
            LATEST_RELEASE_URL_OVERRIDE_ENV_VAR,
            Some("http://example.com"),
        );
        assert_eq!(latest_release_url(), "http://example.com");
    }

    #[test]
    fn fetch_versions_from_servers() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let release_body = "{\"tag_name\":\"v9.9.9\"}";
        let release_resp = http_ok_response(release_body, "application/json");
        let release_url = spawn_server(release_resp);
        {
            let _env = set_env_guard(LATEST_RELEASE_URL_OVERRIDE_ENV_VAR, Some(&release_url));
            assert_eq!(fetch_version_from_release().unwrap(), "9.9.9");
        }

        let cask_body = "version \"9.9.9\"";
        let cask_resp = http_ok_response(cask_body, "text/plain");
        let cask_url = spawn_server(cask_resp);
        {
            let _env = set_env_guard(HOMEBREW_CASK_URL_OVERRIDE_ENV_VAR, Some(&cask_url));
            assert_eq!(fetch_version_from_cask().unwrap(), "9.9.9");
        }
    }

    #[test]
    fn fetch_versions_handle_404() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string();
        let url = spawn_server(resp);
        let _env = set_env_guard(LATEST_RELEASE_URL_OVERRIDE_ENV_VAR, Some(&url));
        assert!(fetch_version_from_release().is_none());
    }

    #[test]
    fn check_for_update_writes_version() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let release_body = "{\"tag_name\":\"v9.9.9\"}";
        let release_resp = http_ok_response(release_body, "application/json");
        let release_url = spawn_server(release_resp);
        let _env = set_env_guard(LATEST_RELEASE_URL_OVERRIDE_ENV_VAR, Some(&release_url));

        let dir = tempfile::tempdir().expect("tempdir");
        let paths = paths_for_update(dir.path().to_path_buf());
        fs::create_dir_all(&paths.profiles).unwrap();
        fs::write(&paths.profiles_lock, "").unwrap();
        check_for_update_with_action(&paths, None).unwrap();
        let contents = fs::read_to_string(&paths.update_cache).unwrap();
        assert!(contents.contains("9.9.9"));
    }

    #[test]
    fn read_update_cache_migrates_legacy_profiles_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = paths_for_update(dir.path().to_path_buf());
        fs::create_dir_all(&paths.profiles).unwrap();
        fs::write(&paths.profiles_lock, "").unwrap();
        let legacy = serde_json::json!({
            "version": 1,
            "profiles": {},
            "update_cache": {
                "latest_version": "1.2.3",
                "last_checked_at": "2024-01-01T00:00:00Z"
            }
        });
        fs::write(
            &paths.profiles_index,
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let migrated = read_update_cache(&paths).unwrap().unwrap();
        assert_eq!(migrated.latest_version, "1.2.3");
        assert!(paths.update_cache.is_file());
        let index_contents = fs::read_to_string(&paths.profiles_index).unwrap();
        assert!(!index_contents.contains("update_cache"));
    }

    #[test]
    fn updates_disabled_variants() {
        let config = UpdateConfig {
            codex_home: PathBuf::new(),
            check_for_update_on_startup: false,
        };
        assert!(updates_disabled_with_debug(&config, false));
        let config = UpdateConfig {
            codex_home: PathBuf::new(),
            check_for_update_on_startup: true,
        };
        assert!(updates_disabled_with_debug(&config, true));
    }

    #[test]
    fn run_update_prompt_paths() {
        let _guard = ENV_MUTEX.lock().unwrap();
        let release_body = format!("{{\"tag_name\":\"v{}\"}}", "99.0.0");
        let release_resp = http_ok_response(&release_body, "application/json");
        let release_url = spawn_server(release_resp);
        let _env = set_env_guard(LATEST_RELEASE_URL_OVERRIDE_ENV_VAR, Some(&release_url));

        let dir = tempfile::tempdir().expect("tempdir");
        let config = UpdateConfig {
            codex_home: dir.path().to_path_buf(),
            check_for_update_on_startup: true,
        };
        seed_version_info(&config, "99.0.0");
        let mut input = std::io::Cursor::new("2\n");
        let mut output = Vec::new();
        let result = run_update_prompt_if_needed_with_io_and_source(
            &config,
            false,
            false,
            InstallSource::Npm,
            &mut input,
            &mut output,
        )
        .unwrap();
        assert!(matches!(result, UpdatePromptOutcome::Continue));

        let cache = read_update_cache(&paths_for_update(config.codex_home.clone()))
            .unwrap()
            .expect("update cache");
        assert!(cache.last_prompted_at.is_none());
        let output = String::from_utf8(output).expect("utf8 output");
        assert!(output.contains("Run `npm install -g codexswitch-cli` to update."));

        let dir = tempfile::tempdir().expect("tempdir");
        let config = UpdateConfig {
            codex_home: dir.path().to_path_buf(),
            check_for_update_on_startup: true,
        };
        seed_version_info(&config, "99.0.0");
        let mut input = std::io::Cursor::new("1\n");
        let mut output = Vec::new();
        let result = run_update_prompt_if_needed_with_io_and_source(
            &config,
            false,
            true,
            InstallSource::Npm,
            &mut input,
            &mut output,
        )
        .unwrap();
        assert!(matches!(result, UpdatePromptOutcome::RunUpdate(_)));

        let cache = read_update_cache(&paths_for_update(config.codex_home.clone()))
            .unwrap()
            .expect("update cache");
        assert!(cache.last_prompted_at.is_some());

        let mut input = std::io::Cursor::new("1\n");
        let mut output = Vec::new();
        let result = run_update_prompt_if_needed_with_io_and_source(
            &config,
            false,
            true,
            InstallSource::Npm,
            &mut input,
            &mut output,
        )
        .unwrap();
        assert!(matches!(result, UpdatePromptOutcome::Continue));
        assert!(output.is_empty());

        let dir = tempfile::tempdir().expect("tempdir");
        let config = UpdateConfig {
            codex_home: dir.path().to_path_buf(),
            check_for_update_on_startup: true,
        };
        seed_version_info(&config, "99.0.0");
        let mut input = std::io::Cursor::new("3\n");
        let mut output = Vec::new();
        let result = run_update_prompt_if_needed_with_io_and_source(
            &config,
            false,
            true,
            InstallSource::Npm,
            &mut input,
            &mut output,
        )
        .unwrap();
        assert!(matches!(result, UpdatePromptOutcome::Continue));
    }
}
