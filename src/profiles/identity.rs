use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{ProfilesIndex, profile_dir_for_id, profile_files, profile_id_from_path};
use crate::{
    AUTH_ERR_INCOMPLETE_ACCOUNT, Paths, ProfileIdentityKey, Tokens, extract_profile_identity,
    read_tokens, require_identity,
};

const SNOWFLAKE_EPOCH_MS: u64 = 1_704_067_200_000;
const SNOWFLAKE_SEQUENCE_BITS: u64 = 12;
const SNOWFLAKE_SEQUENCE_MASK: u64 = (1 << SNOWFLAKE_SEQUENCE_BITS) - 1;
const SNOWFLAKE_NODE_MASK: u64 = (1 << 10) - 1;

static LAST_SNOWFLAKE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn resolve_save_id(
    paths: &Paths,
    _profiles_index: &mut ProfilesIndex,
    tokens: &Tokens,
) -> Result<String, String> {
    let _ = require_identity(tokens)?;
    let identity =
        extract_profile_identity(tokens).ok_or_else(|| AUTH_ERR_INCOMPLETE_ACCOUNT.to_string())?;
    let candidates = scan_profile_ids(&paths.profiles, &identity)?;
    if let Some(primary) = pick_primary(&candidates) {
        return Ok(primary);
    }
    Ok(next_snowflake_profile_id(&paths.profiles))
}

pub(crate) fn resolve_sync_id(
    paths: &Paths,
    _profiles_index: &mut ProfilesIndex,
    tokens: &Tokens,
) -> Result<Option<String>, String> {
    if require_identity(tokens).is_err() {
        return Ok(None);
    };
    let Some(identity) = extract_profile_identity(tokens) else {
        return Ok(None);
    };
    let candidates = scan_profile_ids(&paths.profiles, &identity)?;
    Ok(pick_primary(&candidates))
}

pub(crate) fn cached_profile_ids(
    tokens_map: &BTreeMap<String, Result<Tokens, String>>,
    identity: &ProfileIdentityKey,
) -> Vec<String> {
    tokens_map
        .iter()
        .filter_map(|(id, result)| {
            result
                .as_ref()
                .ok()
                .filter(|tokens| matches_identity(tokens, identity))
                .map(|_| id.clone())
        })
        .collect()
}

pub(crate) fn pick_primary(candidates: &[String]) -> Option<String> {
    candidates.iter().min().cloned()
}

fn scan_profile_ids(
    profiles_dir: &Path,
    identity: &ProfileIdentityKey,
) -> Result<Vec<String>, String> {
    let mut matches = Vec::new();
    for path in profile_files(profiles_dir)? {
        let Ok(tokens) = read_tokens(&path) else {
            continue;
        };
        if !matches_identity(&tokens, identity) {
            continue;
        }
        if let Some(stem) = profile_id_from_path(&path) {
            matches.push(stem);
        }
    }
    Ok(matches)
}

fn matches_identity(tokens: &Tokens, identity: &ProfileIdentityKey) -> bool {
    extract_profile_identity(tokens).is_some_and(|candidate| candidate == *identity)
}

pub(crate) fn next_snowflake_profile_id(profiles_dir: &Path) -> String {
    loop {
        let id = generate_snowflake_id().to_string();
        if !profile_dir_for_id(profiles_dir, &id).exists() {
            return id;
        }
    }
}

fn generate_snowflake_id() -> u64 {
    loop {
        let now_ms = current_snowflake_millis();
        let previous = LAST_SNOWFLAKE.load(Ordering::Relaxed);
        let previous_ms = previous >> SNOWFLAKE_SEQUENCE_BITS;
        let previous_sequence = previous & SNOWFLAKE_SEQUENCE_MASK;
        let (millis, sequence) = if now_ms > previous_ms {
            (now_ms, 0)
        } else if previous_sequence < SNOWFLAKE_SEQUENCE_MASK {
            (previous_ms, previous_sequence + 1)
        } else {
            (previous_ms + 1, 0)
        };
        let next = (millis << SNOWFLAKE_SEQUENCE_BITS) | sequence;
        if LAST_SNOWFLAKE
            .compare_exchange(previous, next, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            let node = u64::from(std::process::id()) & SNOWFLAKE_NODE_MASK;
            return (millis << 22) | (node << SNOWFLAKE_SEQUENCE_BITS) | sequence;
        }
    }
}

fn current_snowflake_millis() -> u64 {
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    unix_ms.saturating_sub(SNOWFLAKE_EPOCH_MS)
}
