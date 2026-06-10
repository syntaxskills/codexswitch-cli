use codex_profiles::{
    InstallSource, detect_install_source_inner, extract_version_from_cask,
    extract_version_from_latest_tag, is_newer,
};

#[test]
fn detects_install_source_without_env_mutation() {
    assert_eq!(
        detect_install_source_inner(false, std::path::Path::new("/any/path"), false, false),
        InstallSource::Unknown
    );
    assert_eq!(
        detect_install_source_inner(false, std::path::Path::new("/any/path"), true, false),
        InstallSource::Npm
    );
    assert_eq!(
        detect_install_source_inner(false, std::path::Path::new("/any/path"), false, true),
        InstallSource::Npm
    );
    assert_eq!(
        detect_install_source_inner(
            false,
            std::path::Path::new(
                "/Users/dev/.bun/install/global/node_modules/codexswitch-cli/bin/codexswitch-cli",
            ),
            false,
            true,
        ),
        InstallSource::Bun
    );
    assert_eq!(
        detect_install_source_inner(
            true,
            std::path::Path::new("/opt/homebrew/bin/codexswitch-cli"),
            false,
            false,
        ),
        InstallSource::Brew
    );
    assert_eq!(
        detect_install_source_inner(
            true,
            std::path::Path::new("/usr/local/bin/codexswitch-cli"),
            false,
            false,
        ),
        InstallSource::Brew
    );
}

#[test]
fn parses_version_from_cask_contents() {
    let cask = r#"
        cask "codexswitch-cli" do
          version "0.55.0"
        end
    "#;
    assert_eq!(
        extract_version_from_cask(cask).expect("failed to parse version"),
        "0.55.0"
    );
}

#[test]
fn extracts_version_from_latest_tag() {
    assert_eq!(
        extract_version_from_latest_tag("rust-v1.5.0").expect("failed to parse version"),
        "1.5.0"
    );
    assert_eq!(
        extract_version_from_latest_tag("v1.5.0").expect("failed to parse version"),
        "1.5.0"
    );
}

#[test]
fn latest_tag_without_prefix_is_invalid() {
    assert!(extract_version_from_latest_tag("codex-v1.5.0").is_err());
}

#[test]
fn plain_semver_comparisons_work() {
    assert_eq!(is_newer("0.55.0", "0.54.0"), Some(true));
    assert_eq!(is_newer("0.55.0", "0.55.0"), Some(false));
    assert_eq!(is_newer("0.55.0", "0.56.0"), Some(false));
}

#[test]
fn prerelease_version_is_not_considered_newer() {
    assert_eq!(is_newer("0.55.0-alpha.1", "0.55.0"), None);
}

#[test]
fn whitespace_is_ignored() {
    assert_eq!(is_newer(" 0.55.0 ", "0.54.0"), Some(true));
}
