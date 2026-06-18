mod common;

use codexswitch_cli::{AuthFile, Tokens, extract_email_and_plan};
use common::build_id_token;

#[test]
fn extracts_email_and_plan_from_id_token() {
    let id_token = build_id_token("alpha@example.com", "team");
    let tokens = Tokens {
        account_id: None,
        id_token: Some(id_token),
        access_token: None,
        refresh_token: None,
    };
    let (email, plan) = extract_email_and_plan(&tokens);
    assert_eq!(email.as_deref(), Some("alpha@example.com"));
    assert_eq!(plan.as_deref(), Some("Team"));
}

#[test]
fn extracts_email_and_plan_for_api_key_profile() {
    let tokens = Tokens {
        account_id: Some("api-key-sk-proj-a3x~1234".to_string()),
        id_token: None,
        access_token: None,
        refresh_token: None,
    };
    let (email, plan) = extract_email_and_plan(&tokens);
    assert_eq!(email.as_deref(), Some("~1234"));
    assert_eq!(plan.as_deref(), Some("Key"));
}

#[test]
fn extracts_email_and_plan_for_legacy_api_key_profile() {
    let tokens = Tokens {
        account_id: Some("api-key-1234".to_string()),
        id_token: None,
        access_token: None,
        refresh_token: None,
    };
    let (email, plan) = extract_email_and_plan(&tokens);
    assert_eq!(email.as_deref(), Some("Key"));
    assert_eq!(plan.as_deref(), Some("Key"));
}

#[test]
fn auth_file_parses_without_refresh_token() {
    let id_token = build_id_token("alpha@example.com", "team");
    let json = format!(
        "{{\"tokens\":{{\"account_id\":\"acct-alpha\",\"id_token\":\"{id_token}\",\"access_token\":\"token\"}}}}"
    );
    let auth: AuthFile = serde_json::from_str(&json).expect("parse auth");
    let tokens = auth.tokens.expect("tokens");
    assert_eq!(tokens.account_id.as_deref(), Some("acct-alpha"));
}
