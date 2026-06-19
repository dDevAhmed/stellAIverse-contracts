#![no_std]

mod storage;
pub mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};
use stellai_lib::{admin, ADMIN_KEY};

use storage::*;
use types::*;

#[contract]
pub struct MetricsAggregator;

#[contractimpl]
impl MetricsAggregator {
    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    /// Initialize the metrics aggregator (one-time setup)
    pub fn init_contract(env: Env, admin_addr: Address) {
        let existing = env
            .storage()
            .instance()
            .get::<_, Address>(&Symbol::new(&env, ADMIN_KEY));
        if existing.is_some() {
            panic!("Contract already initialized");
        }

        admin_addr.require_auth();
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &admin_addr);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, BUCKET_COUNTER_KEY), &0u64);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, SNAPSHOT_COUNTER_KEY), &0u64);

        env.events()
            .publish((Symbol::new(&env, "metrics_init"),), (admin_addr,));
    }

    // ========================================================================
    // METRIC RECORDING
    // ========================================================================

    /// Record a metric data point. Automatically aggregates into hourly, daily,
    /// and monthly buckets. Admin-only.
    ///
    /// # Arguments
    /// * `caller` – Must be admin
    /// * `metric_type` – Which metric to record
    /// * `value` – The metric value (e.g., 1 for a count, or a price amount)
    /// * `timestamp` – Ledger timestamp of the event
    pub fn record_metric(
        env: Env,
        caller: Address,
        metric_type: MetricType,
        value: i128,
        timestamp: u64,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        // Update cumulative counter
        add_cumulative(&env, metric_type, value);

        // Insert/update bucket for each granularity
        Self::upsert_bucket(&env, metric_type, BucketDuration::Hourly, value, timestamp);
        Self::upsert_bucket(&env, metric_type, BucketDuration::Daily, value, timestamp);
        Self::upsert_bucket(&env, metric_type, BucketDuration::Monthly, value, timestamp);

        env.events().publish(
            (Symbol::new(&env, "metric_recorded"),),
            (metric_type as u32, value, timestamp),
        );
    }

    /// Record user activity. Increments the appropriate field in UserStats.
    /// Admin-only.
    pub fn record_user_activity(
        env: Env,
        caller: Address,
        user: Address,
        activity_type: UserActivityType,
        value: i128,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let now = env.ledger().timestamp();
        let mut stats = storage::get_user_stats(&env, &user).unwrap_or(UserStats {
            user: user.clone(),
            agents_owned: 0,
            agents_traded: 0,
            agents_leased: 0,
            total_volume: 0,
            total_spent: 0,
            participation_score: 0,
            last_active: 0,
        });

        stats.last_active = now;

        match activity_type {
            UserActivityType::AgentOwned => {
                stats.agents_owned = stats.agents_owned.saturating_add(value as u32);
            }
            UserActivityType::AgentTraded => {
                stats.agents_traded = stats.agents_traded.saturating_add(value as u32);
            }
            UserActivityType::AgentLeased => {
                stats.agents_leased = stats.agents_leased.saturating_add(value as u32);
            }
            UserActivityType::VolumeAdded => {
                stats.total_volume = stats.total_volume.saturating_add(value);
            }
            UserActivityType::AmountSpent => {
                stats.total_spent = stats.total_spent.saturating_add(value);
            }
            UserActivityType::ParticipationScored => {
                stats.participation_score = stats.participation_score.saturating_add(value as u32);
            }
        }

        storage::store_user_stats(&env, &stats);

        env.events().publish(
            (Symbol::new(&env, "user_activity"),),
            (user, activity_type as u32, value),
        );
    }

    /// Update an agent's score for top-N ranking. Admin-only.
    pub fn update_agent_score(
        env: Env,
        caller: Address,
        agent_id: u64,
        order_by: OrderBy,
        score: i128,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        storage::store_agent_score(&env, agent_id, order_by, score);
        storage::add_agent_to_scoreboard(&env, order_by, agent_id);

        env.events().publish(
            (Symbol::new(&env, "agent_score"),),
            (agent_id, order_by as u32, score),
        );
    }

    // ========================================================================
    // QUERIES
    // ========================================================================

    /// Query aggregated metrics for a given type and duration within a time range.
    ///
    /// Iterates bucket indices for each aligned timestamp in [start_time, end_time].
    pub fn query_metrics(
        env: Env,
        metric_type: MetricType,
        bucket_duration: BucketDuration,
        start_time: u64,
        end_time: u64,
        limit: u32,
    ) -> MetricsQueryResult {
        let effective_limit = if limit == 0 || limit > MAX_QUERY_LIMIT {
            MAX_QUERY_LIMIT
        } else {
            limit
        };

        let period = duration_seconds(bucket_duration);
        let aligned_start = align_timestamp(start_time, bucket_duration);
        let aligned_end = align_timestamp(end_time, bucket_duration);

        let mut buckets: Vec<MetricsBucket> = Vec::new(&env);
        let mut count: u32 = 0;
        let mut ts = aligned_start;

        while ts <= aligned_end && count < effective_limit {
            if let Some(bid) = get_bucket_index(&env, metric_type, bucket_duration, ts) {
                if let Some(bucket) = get_bucket(&env, bid) {
                    buckets.push_back(bucket);
                    count += 1;
                }
            }
            // Advance to next period; guard against zero period
            ts = ts.saturating_add(period);
            if period == 0 {
                break;
            }
        }

        let has_more = ts <= aligned_end && count == effective_limit;

        MetricsQueryResult {
            buckets,
            total_count: count,
            has_more,
        }
    }

    /// Get analytics summary for a specific user.
    pub fn get_user_stats(env: Env, user: Address) -> Option<UserStats> {
        storage::get_user_stats(&env, &user)
    }

    /// Get top-N agents ranked by the specified criterion.
    pub fn get_top_agents(env: Env, order_by: OrderBy, limit: u32) -> Vec<AgentRanking> {
        let effective_limit = if limit == 0 || limit > MAX_QUERY_LIMIT {
            MAX_QUERY_LIMIT
        } else {
            limit
        };

        let agent_ids = storage::get_agent_scoreboard(&env, order_by);
        let mut rankings: Vec<AgentRanking> = Vec::new(&env);

        // Collect all scores
        for i in 0..agent_ids.len() {
            if let Some(aid) = agent_ids.get(i) {
                let score = storage::get_agent_score(&env, aid, order_by).unwrap_or(0);
                rankings.push_back(AgentRanking {
                    agent_id: aid,
                    score,
                });
            }
        }

        // Simple insertion sort descending (suitable for on-chain with bounded data)
        let len = rankings.len();
        if len > 1 {
            let mut i = 1u32;
            while i < len {
                let current = rankings.get(i).unwrap();
                let mut j = i;
                while j > 0 {
                    let prev = rankings.get(j - 1).unwrap();
                    if prev.score < current.score {
                        rankings.set(j, prev);
                        j -= 1;
                    } else {
                        break;
                    }
                }
                rankings.set(j, current);
                i += 1;
            }
        }

        // Truncate to limit
        let mut result: Vec<AgentRanking> = Vec::new(&env);
        let take = if len < effective_limit {
            len
        } else {
            effective_limit
        };
        for i in 0..take {
            if let Some(r) = rankings.get(i) {
                result.push_back(r);
            }
        }

        result
    }

    // ========================================================================
    // PORTFOLIO ANALYTICS
    // ========================================================================

    /// Record a portfolio snapshot for a user. Admin-only.
    pub fn record_portfolio_snapshot(
        env: Env,
        caller: Address,
        user: Address,
        value: i128,
        realized_pnl: i128,
        unrealized_pnl: i128,
    ) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let snapshot = PortfolioSnapshot {
            timestamp: env.ledger().timestamp(),
            value,
            realized_pnl,
            unrealized_pnl,
        };

        let key = (Symbol::new(&env, "portfolio"), user.clone());
        let mut snapshots: Vec<PortfolioSnapshot> =
            env.storage().instance().get(&key).unwrap_or(Vec::new(&env));
        snapshots.push_back(snapshot);
        env.storage().instance().set(&key, &snapshots);

        env.events()
            .publish((Symbol::new(&env, "portfolio_snapshot"),), (user, value));
    }

    /// Record a trade for a user. Admin-only.
    pub fn record_trade(env: Env, caller: Address, user: Address, pnl: i128, size: i128) {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let trade = Trade {
            timestamp: env.ledger().timestamp(),
            pnl,
            size,
        };

        let key = (Symbol::new(&env, "trades"), user.clone());
        let mut trades: Vec<Trade> = env.storage().instance().get(&key).unwrap_or(Vec::new(&env));
        trades.push_back(trade);
        env.storage().instance().set(&key, &trades);

        env.events()
            .publish((Symbol::new(&env, "trade_recorded"),), (user, pnl));
    }

    /// Get analytics summary for a user.
    pub fn get_analytics_summary(env: Env, user: Address) -> AnalyticsSummary {
        let trades_key = (Symbol::new(&env, "trades"), user.clone());
        let trades: Vec<Trade> = env
            .storage()
            .instance()
            .get(&trades_key)
            .unwrap_or(Vec::new(&env));

        let mut total_pnl = 0i128;
        let mut wins = 0u32;
        let mut total_trades = 0u32;
        let mut _total_profit = 0i128;

        for i in 0..trades.len() {
            if let Some(t) = trades.get(i) {
                total_pnl += t.pnl;
                if t.pnl > 0 {
                    wins += 1;
                    // total_profit += t.pnl; // commented out because unused - clippy warning
                }
                total_trades += 1;
            }
        }

        let win_rate = if total_trades > 0 {
            (wins as i128 * 100) / total_trades as i128
        } else {
            0
        };

        let avg_trade_profit = if total_trades > 0 {
            total_pnl / total_trades as i128
        } else {
            0
        };

        // Sharpe ratio calculation (simplified)
        let portfolio_key = (Symbol::new(&env, "portfolio"), user.clone());
        let snapshots: Vec<PortfolioSnapshot> = env
            .storage()
            .instance()
            .get(&portfolio_key)
            .unwrap_or(Vec::new(&env));

        let mut returns: Vec<i128> = Vec::new(&env);
        if snapshots.len() > 1 {
            for i in 1..snapshots.len() {
                if let Some(curr) = snapshots.get(i) {
                    if let Some(prev) = snapshots.get(i - 1) {
                        if prev.value > 0 {
                            let ret = ((curr.value - prev.value) * 10000) / prev.value; // scaled
                            returns.push_back(ret);
                        }
                    }
                }
            }
        }

        let mut sum_returns = 0i128;
        let mut sum_sq = 0i128;
        let n = returns.len() as i128;

        for i in 0..returns.len() {
            if let Some(r) = returns.get(i) {
                sum_returns += r;
                sum_sq += r * r;
            }
        }

        let avg_return = if n > 0 { sum_returns / n } else { 0 };
        let variance = if n > 1 {
            (sum_sq * n - sum_returns * sum_returns) / (n * (n - 1))
        } else {
            0
        };
        let std_dev = if variance > 0 {
            // Approximate sqrt for i128
            let mut x = variance;
            let mut y = (x + 1) / 2;
            while y < x {
                x = y;
                y = (x + variance / x) / 2;
            }
            x
        } else {
            0
        };

        let sharpe_ratio = if std_dev > 0 {
            (avg_return * 100) / std_dev
        } else {
            0
        };

        let current_value = if !snapshots.is_empty() {
            if let Some(last) = snapshots.last() {
                last.value
            } else {
                0
            }
        } else {
            0
        };

        AnalyticsSummary {
            total_pnl,
            win_rate,
            avg_trade_profit,
            sharpe_ratio,
            current_value,
        }
    }

    // ========================================================================
    // SNAPSHOTS
    // ========================================================================

    /// Take a point-in-time snapshot of platform-wide metrics. Admin-only.
    pub fn take_snapshot(
        env: Env,
        caller: Address,
        total_agents: u64,
        active_listings: u64,
        total_volume: i128,
        total_sales: u64,
        total_evolutions: u64,
        active_proposals: u32,
    ) -> u64 {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let snapshot_id = increment_counter(&env, SNAPSHOT_COUNTER_KEY);
        let snapshot = MetricSnapshot {
            snapshot_id,
            timestamp: env.ledger().timestamp(),
            total_agents,
            active_listings,
            total_volume,
            total_sales,
            total_evolutions,
            active_proposals,
        };

        storage::store_snapshot(&env, &snapshot);

        env.events()
            .publish((Symbol::new(&env, "snapshot_taken"),), (snapshot_id,));

        snapshot_id
    }

    /// Retrieve a specific snapshot by ID.
    pub fn get_snapshot(env: Env, snapshot_id: u64) -> Option<MetricSnapshot> {
        storage::get_snapshot(&env, snapshot_id)
    }

    // ========================================================================
    // PRUNING
    // ========================================================================

    /// Prune old metric buckets to reclaim storage. Admin-only.
    ///
    /// Deletes hourly buckets older than `RETENTION_HOURLY_SECONDS` and
    /// daily buckets older than `RETENTION_DAILY_SECONDS` relative to
    /// `before_timestamp`. Monthly buckets are never pruned.
    ///
    /// Returns the number of buckets removed.
    pub fn prune_metrics(env: Env, caller: Address, before_timestamp: u64) -> u32 {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        let total_buckets = get_counter(&env, BUCKET_COUNTER_KEY);
        let mut pruned = 0u32;

        for bid in 1..=total_buckets {
            if let Some(bucket) = get_bucket(&env, bid) {
                let should_prune = match bucket.duration {
                    BucketDuration::Hourly => {
                        before_timestamp.saturating_sub(bucket.timestamp) > RETENTION_HOURLY_SECONDS
                    }
                    BucketDuration::Daily => {
                        before_timestamp.saturating_sub(bucket.timestamp) > RETENTION_DAILY_SECONDS
                    }
                    BucketDuration::Monthly => false, // Never prune monthly
                };

                if should_prune {
                    remove_bucket_index(
                        &env,
                        bucket.metric_type,
                        bucket.duration,
                        bucket.timestamp,
                    );
                    remove_bucket(&env, bid);
                    pruned += 1;
                }
            }
        }

        env.events().publish(
            (Symbol::new(&env, "metrics_pruned"),),
            (pruned, before_timestamp),
        );

        pruned
    }

    // ========================================================================
    // PLATFORM SUMMARY
    // ========================================================================

    /// Get a platform-wide summary from cumulative counters.
    pub fn get_platform_summary(env: Env) -> PlatformSummary {
        PlatformSummary {
            timestamp: env.ledger().timestamp(),
            total_agents_minted: get_cumulative(&env, MetricType::AgentsMinted),
            total_marketplace_sales: get_cumulative(&env, MetricType::MarketplaceSales),
            total_marketplace_volume: get_cumulative(&env, MetricType::MarketplaceVolume),
            total_execution_actions: get_cumulative(&env, MetricType::ExecutionActions),
            total_evolution_requests: get_cumulative(&env, MetricType::EvolutionRequests),
            total_evolution_completed: get_cumulative(&env, MetricType::EvolutionCompleted),
            total_governance_proposals: get_cumulative(&env, MetricType::GovernanceProposals),
        }
    }

    // ========================================================================
    // INTERNAL HELPERS
    // ========================================================================

    fn verify_admin(env: &Env, caller: &Address) {
        if admin::verify_admin(env, caller).is_err() {
            panic!("Unauthorized: caller is not admin");
        }
    }

    /// Insert or update a bucket for the given metric, duration, and timestamp.
    fn upsert_bucket(
        env: &Env,
        metric_type: MetricType,
        duration: BucketDuration,
        value: i128,
        timestamp: u64,
    ) {
        let aligned_ts = align_timestamp(timestamp, duration);

        if let Some(existing_id) = get_bucket_index(env, metric_type, duration, aligned_ts) {
            // Update existing bucket
            if let Some(mut bucket) = get_bucket(env, existing_id) {
                bucket.value = bucket.value.saturating_add(value);
                bucket.count = bucket.count.saturating_add(1);
                if value < bucket.min {
                    bucket.min = value;
                }
                if value > bucket.max {
                    bucket.max = value;
                }
                store_bucket(env, &bucket);
            }
        } else {
            // Create new bucket
            let bucket_id = increment_counter(env, BUCKET_COUNTER_KEY);
            let bucket = MetricsBucket {
                bucket_id,
                timestamp: aligned_ts,
                duration,
                metric_type,
                value,
                count: 1,
                min: value,
                max: value,
            };
            store_bucket(env, &bucket);
            set_bucket_index(env, metric_type, duration, aligned_ts, bucket_id);
        }
    }

    // ========================================================================
    // REPUTATION & FEEDBACK APIs
    // ========================================================================

    /// Submit feedback about an `agent_id`. Any caller may submit feedback;
    /// the reporter must `require_auth`. Feedback is aggregated immediately
    /// into a compact EWMA reputation score (denominator = 8).
    pub fn submit_feedback(
        env: Env,
        reporter: Address,
        agent_id: u64,
        value: i128,
        reason: ReputationReason,
    ) -> u64 {
        reporter.require_auth();
        let now = env.ledger().timestamp();

        let fb_id = increment_counter(&env, FEEDBACK_COUNTER_KEY);
        let fb = Feedback {
            feedback_id: fb_id,
            reporter: reporter.clone(),
            agent_id,
            value,
            reason,
            timestamp: now,
            resolved: false,
            dispute_id: 0,
        };

        store_feedback(&env, &fb);
        add_feedback_to_agent(&env, agent_id, fb_id);

        // Update (or create) aggregated reputation using a simple EWMA:
        // new = ((den-1) * old + value) / den, den=8
        let den: i128 = 8;
        let mut rep = get_reputation(&env, agent_id).unwrap_or(AgentReputation {
            agent_id,
            score: value,
            count: 1,
            last_updated: now,
        });

        if rep.count == 0 {
            rep.score = value;
            rep.count = 1;
        } else {
            let old = rep.score;
            let next = (old.saturating_mul(den - 1).saturating_add(value)) / den;
            rep.score = next;
            rep.count = rep.count.saturating_add(1);
            rep.last_updated = now;
        }

        store_reputation(&env, &rep);

        env.events()
            .publish((Symbol::new(&env, "feedback_submitted"),), (fb_id,));

        fb_id
    }

    /// Retrieve current aggregated reputation for an agent (if any)
    pub fn get_reputation(env: Env, agent_id: u64) -> Option<AgentReputation> {
        get_reputation(&env, agent_id)
    }

    /// Submit a dispute about an existing feedback entry. Reporter must auth.
    pub fn submit_dispute(env: Env, reporter: Address, feedback_id: u64) -> u64 {
        reporter.require_auth();
        let now = env.ledger().timestamp();
        let did = increment_counter(&env, DISPUTE_COUNTER_KEY);
        let dispute = Dispute {
            dispute_id: did,
            feedback_id,
            reporter: reporter.clone(),
            timestamp: now,
            resolved: false,
            outcome: false,
        };

        store_dispute(&env, &dispute);
        env.events()
            .publish((Symbol::new(&env, "dispute_submitted"),), (did,));
        did
    }

    /// Resolve a dispute (admin-only). If `upheld` is true, the disputed
    /// feedback is considered invalid and a penalty is applied to reputation.
    pub fn resolve_dispute(env: Env, caller: Address, dispute_id: u64, upheld: bool) -> bool {
        caller.require_auth();
        Self::verify_admin(&env, &caller);

        if let Some(mut d) = get_dispute(&env, dispute_id) {
            if d.resolved {
                return d.outcome;
            }
            d.resolved = true;
            d.outcome = upheld;
            store_dispute(&env, &d);

            if upheld {
                // Apply penalty: reduce agent reputation by feedback.value (clamped)
                if let Some(mut fb) = get_feedback(&env, d.feedback_id) {
                    if !fb.resolved {
                        fb.resolved = true;
                        fb.dispute_id = dispute_id;
                        store_feedback(&env, &fb);

                        if let Some(mut rep) = get_reputation(&env, fb.agent_id) {
                            // Simple penalty: subtract half of the feedback value
                            let penalty = fb.value / 2;
                            rep.score = rep.score.saturating_sub(penalty);
                            rep.last_updated = env.ledger().timestamp();
                            store_reputation(&env, &rep);
                        }
                    }
                }
            } else {
                // Not upheld: mark feedback resolved but no penalty
                if let Some(mut fb) = get_feedback(&env, d.feedback_id) {
                    if !fb.resolved {
                        fb.resolved = true;
                        fb.dispute_id = dispute_id;
                        store_feedback(&env, &fb);
                    }
                }
            }

            env.events().publish(
                (Symbol::new(&env, "dispute_resolved"),),
                (dispute_id, upheld),
            );
            return upheld;
        }

        false
    }
}
