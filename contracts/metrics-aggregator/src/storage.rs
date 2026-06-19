use soroban_sdk::{Address, Env, Symbol};

use crate::types::{BucketDuration, MetricSnapshot, MetricType, MetricsBucket, OrderBy, UserStats};

// ============================================================================
// STORAGE KEY CONSTANTS
// ============================================================================

pub const BUCKET_COUNTER_KEY: &str = "bucket_ctr";
pub const SNAPSHOT_COUNTER_KEY: &str = "snap_ctr";
/// Retention: keep 1 year of hourly data (365 * 24 * 3600)
pub const RETENTION_HOURLY_SECONDS: u64 = 31_536_000;

/// Retention: keep 2 years of daily data (730 * 24 * 3600)
pub const RETENTION_DAILY_SECONDS: u64 = 63_072_000;

/// Maximum number of results per query
pub const MAX_QUERY_LIMIT: u32 = 100;

/// Duration constants for bucket alignment
pub const HOURLY_SECONDS: u64 = 3600;
pub const DAILY_SECONDS: u64 = 86400;
pub const MONTHLY_SECONDS: u64 = 2_592_000; // 30 days

// ---------------------------------------------------------------------------
// Reputation keys & counters
// ---------------------------------------------------------------------------
pub const FEEDBACK_COUNTER_KEY: &str = "fb_ctr";
pub const DISPUTE_COUNTER_KEY: &str = "dispute_ctr";

// ============================================================================
// COUNTER HELPERS
// ============================================================================

pub fn get_counter(env: &Env, key: &str) -> u64 {
    env.storage()
        .instance()
        .get::<_, u64>(&Symbol::new(env, key))
        .unwrap_or(0)
}

pub fn increment_counter(env: &Env, key: &str) -> u64 {
    let next = get_counter(env, key).saturating_add(1);
    env.storage().instance().set(&Symbol::new(env, key), &next);
    next
}

// ============================================================================
// BUCKET STORAGE
// ============================================================================

pub fn store_bucket(env: &Env, bucket: &MetricsBucket) {
    let key = (Symbol::new(env, "mbucket"), bucket.bucket_id);
    env.storage().persistent().set(&key, bucket);
}

pub fn get_bucket(env: &Env, bucket_id: u64) -> Option<MetricsBucket> {
    let key = (Symbol::new(env, "mbucket"), bucket_id);
    env.storage().persistent().get(&key)
}

pub fn remove_bucket(env: &Env, bucket_id: u64) {
    let key = (Symbol::new(env, "mbucket"), bucket_id);
    env.storage().persistent().remove(&key);
}

// ============================================================================
// BUCKET INDEX — maps (metric_type, duration, aligned_timestamp) → bucket_id
// ============================================================================

pub fn set_bucket_index(
    env: &Env,
    metric_type: MetricType,
    duration: BucketDuration,
    aligned_ts: u64,
    bucket_id: u64,
) {
    let key = (
        Symbol::new(env, "mbidx"),
        metric_type as u32,
        duration as u32,
        aligned_ts,
    );
    env.storage().persistent().set(&key, &bucket_id);
}

pub fn get_bucket_index(
    env: &Env,
    metric_type: MetricType,
    duration: BucketDuration,
    aligned_ts: u64,
) -> Option<u64> {
    let key = (
        Symbol::new(env, "mbidx"),
        metric_type as u32,
        duration as u32,
        aligned_ts,
    );
    env.storage().persistent().get(&key)
}

pub fn remove_bucket_index(
    env: &Env,
    metric_type: MetricType,
    duration: BucketDuration,
    aligned_ts: u64,
) {
    let key = (
        Symbol::new(env, "mbidx"),
        metric_type as u32,
        duration as u32,
        aligned_ts,
    );
    env.storage().persistent().remove(&key);
}

// ============================================================================
// SNAPSHOT STORAGE
// ============================================================================

pub fn store_snapshot(env: &Env, snapshot: &MetricSnapshot) {
    let key = (Symbol::new(env, "msnap"), snapshot.snapshot_id);
    env.storage().persistent().set(&key, snapshot);
}

pub fn get_snapshot(env: &Env, snapshot_id: u64) -> Option<MetricSnapshot> {
    let key = (Symbol::new(env, "msnap"), snapshot_id);
    env.storage().persistent().get(&key)
}

// ============================================================================
// USER STATS STORAGE
// ============================================================================

pub fn store_user_stats(env: &Env, stats: &UserStats) {
    let key = (Symbol::new(env, "ustats"), stats.user.clone());
    env.storage().persistent().set(&key, stats);
}

pub fn get_user_stats(env: &Env, user: &Address) -> Option<UserStats> {
    let key = (Symbol::new(env, "ustats"), user.clone());
    env.storage().persistent().get(&key)
}

// ============================================================================
// AGENT SCORE STORAGE
// ============================================================================

pub fn store_agent_score(env: &Env, agent_id: u64, order_by: OrderBy, score: i128) {
    let key = (Symbol::new(env, "ascore"), agent_id, order_by as u32);
    env.storage().persistent().set(&key, &score);
}

pub fn get_agent_score(env: &Env, agent_id: u64, order_by: OrderBy) -> Option<i128> {
    let key = (Symbol::new(env, "ascore"), agent_id, order_by as u32);
    env.storage().persistent().get(&key)
}

/// Store a scored agent_id in an indexed list for top-N retrieval
pub fn add_agent_to_scoreboard(env: &Env, order_by: OrderBy, agent_id: u64) {
    let list_key = (Symbol::new(env, "asc_list"), order_by as u32);
    let mut list: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&list_key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    // Avoid duplicates
    for i in 0..list.len() {
        if list.get(i) == Some(agent_id) {
            return;
        }
    }
    list.push_back(agent_id);
    env.storage().persistent().set(&list_key, &list);
}

pub fn get_agent_scoreboard(env: &Env, order_by: OrderBy) -> soroban_sdk::Vec<u64> {
    let list_key = (Symbol::new(env, "asc_list"), order_by as u32);
    env.storage()
        .persistent()
        .get(&list_key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

// ============================================================================
// REPUTATION & FEEDBACK STORAGE
// ============================================================================

pub fn store_reputation(env: &Env, rep: &crate::types::AgentReputation) {
    let key = (Symbol::new(env, "rep"), rep.agent_id);
    env.storage().persistent().set(&key, rep);
}

pub fn get_reputation(env: &Env, agent_id: u64) -> Option<crate::types::AgentReputation> {
    let key = (Symbol::new(env, "rep"), agent_id);
    env.storage().persistent().get(&key)
}

pub fn store_feedback(env: &Env, fb: &crate::types::Feedback) {
    let key = (Symbol::new(env, "fb"), fb.feedback_id);
    env.storage().persistent().set(&key, fb);
}

pub fn get_feedback(env: &Env, feedback_id: u64) -> Option<crate::types::Feedback> {
    let key = (Symbol::new(env, "fb"), feedback_id);
    env.storage().persistent().get(&key)
}

pub fn add_feedback_to_agent(env: &Env, agent_id: u64, feedback_id: u64) {
    let list_key = (Symbol::new(env, "fb_list"), agent_id);
    let mut list: soroban_sdk::Vec<u64> = env
        .storage()
        .persistent()
        .get(&list_key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    // Avoid duplicates
    for i in 0..list.len() {
        if list.get(i) == Some(feedback_id) {
            return;
        }
    }
    list.push_back(feedback_id);
    env.storage().persistent().set(&list_key, &list);
}

pub fn _get_feedback_ids_for_agent(env: &Env, agent_id: u64) -> soroban_sdk::Vec<u64> {
    let list_key = (Symbol::new(env, "fb_list"), agent_id);
    env.storage()
        .persistent()
        .get(&list_key)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

pub fn store_dispute(env: &Env, d: &crate::types::Dispute) {
    let key = (Symbol::new(env, "dispute"), d.dispute_id);
    env.storage().persistent().set(&key, d);
}

pub fn get_dispute(env: &Env, dispute_id: u64) -> Option<crate::types::Dispute> {
    let key = (Symbol::new(env, "dispute"), dispute_id);
    env.storage().persistent().get(&key)
}

// ============================================================================
// CUMULATIVE COUNTER STORAGE (for platform summary)
// ============================================================================

pub fn get_cumulative(env: &Env, metric_type: MetricType) -> i128 {
    let key = (Symbol::new(env, "mcum"), metric_type as u32);
    env.storage().persistent().get::<_, i128>(&key).unwrap_or(0)
}

pub fn add_cumulative(env: &Env, metric_type: MetricType, value: i128) {
    let current = get_cumulative(env, metric_type);
    let next = current.saturating_add(value);
    let key = (Symbol::new(env, "mcum"), metric_type as u32);
    env.storage().persistent().set(&key, &next);
}

// ============================================================================
// ALIGNMENT HELPERS
// ============================================================================

/// Align a timestamp down to the start of its bucket period
pub fn align_timestamp(timestamp: u64, duration: BucketDuration) -> u64 {
    let period = duration_seconds(duration);
    if period == 0 {
        return timestamp;
    }
    (timestamp / period) * period
}

/// Get the number of seconds for a bucket duration
pub fn duration_seconds(duration: BucketDuration) -> u64 {
    match duration {
        BucketDuration::Hourly => HOURLY_SECONDS,
        BucketDuration::Daily => DAILY_SECONDS,
        BucketDuration::Monthly => MONTHLY_SECONDS,
    }
}
