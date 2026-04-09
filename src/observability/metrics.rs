//! Prometheus metrics for the coordinator (SPEC-11 R10-R18).
//!
//! Feature-gated under `metrics`. When not enabled, this module
//! is not compiled at all.

use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicI64;

/// Label value for the `rule` dimension on `interactions_by_rule_total` (SPEC-11 R12, R16).
///
/// Six variants matching the IC interaction rules. Max cardinality = 6.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct RuleLabel {
    /// The IC interaction rule name.
    pub rule: RuleValue,
}

/// Possible values for the `rule` label (SPEC-11 R16).
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelValue)]
pub enum RuleValue {
    /// CON-CON annihilation.
    ConCon,
    /// CON-DUP commutation.
    ConDup,
    /// CON-ERA erasure.
    ConEra,
    /// DUP-DUP annihilation.
    DupDup,
    /// DUP-ERA erasure.
    DupEra,
    /// ERA-ERA annihilation.
    EraEra,
}

/// All six rule labels, indexed by `SpecificRule` ordinal (0..6).
pub const RULE_LABELS: [RuleValue; 6] = [
    RuleValue::ConCon,
    RuleValue::ConDup,
    RuleValue::ConEra,
    RuleValue::DupDup,
    RuleValue::DupEra,
    RuleValue::EraEra,
];

/// Custom histogram buckets tuned to IC reduction latencies (SPEC-11 R15).
///
/// Default Prometheus buckets (for web requests) are inappropriate;
/// IC reduction rounds can range from sub-millisecond to tens of seconds.
const HISTOGRAM_BUCKETS: [f64; 9] = [0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 30.0];

/// Coordinator-side Prometheus metrics (SPEC-11 R12).
///
/// All metric names are prefixed with `relativist_` (R17).
/// Metrics are registered once via `register()` and updated
/// directly by the coordinator during the grid cycle.
#[derive(Debug)]
pub struct CoordinatorMetrics {
    /// Total BSP rounds completed.
    pub rounds_total: Counter,
    /// Wall-clock duration of each round (seconds).
    pub round_duration_seconds: Histogram,
    /// Number of currently connected workers.
    pub active_workers: Gauge<i64, AtomicI64>,
    /// Total partitions dispatched across all rounds.
    pub partitions_dispatched_total: Counter,
    /// Border redexes detected after last merge.
    pub border_redexes: Gauge<i64, AtomicI64>,
    /// Duration of the merge phase (seconds).
    pub merge_duration_seconds: Histogram,
    /// Duration of the split phase (seconds).
    pub split_duration_seconds: Histogram,
    /// Total bytes dispatched to workers.
    pub dispatch_bytes_total: Counter,
    /// Total bytes received back from workers.
    pub return_bytes_total: Counter,
    /// Total interactions by rule type, across all workers and rounds (SPEC-11 R12).
    ///
    /// Labeled by `RuleLabel` with 6 variants (R16: max cardinality = 6).
    pub interactions_by_rule_total: Family<RuleLabel, Counter>,
}

impl CoordinatorMetrics {
    /// Create metrics and register them in the given registry (SPEC-11 R18).
    pub fn register(registry: &mut Registry) -> Self {
        let rounds_total = Counter::default();
        registry.register(
            "relativist_rounds_total",
            "Total BSP rounds completed",
            rounds_total.clone(),
        );

        let round_duration_seconds = Histogram::new(HISTOGRAM_BUCKETS);
        registry.register(
            "relativist_round_duration_seconds",
            "Wall-clock duration of each round",
            round_duration_seconds.clone(),
        );

        let active_workers = Gauge::<i64, AtomicI64>::default();
        registry.register(
            "relativist_active_workers",
            "Number of currently connected workers",
            active_workers.clone(),
        );

        let partitions_dispatched_total = Counter::default();
        registry.register(
            "relativist_partitions_dispatched_total",
            "Total partitions dispatched across all rounds",
            partitions_dispatched_total.clone(),
        );

        let border_redexes = Gauge::<i64, AtomicI64>::default();
        registry.register(
            "relativist_border_redexes",
            "Border redexes detected after last merge",
            border_redexes.clone(),
        );

        let merge_duration_seconds = Histogram::new(HISTOGRAM_BUCKETS);
        registry.register(
            "relativist_merge_duration_seconds",
            "Duration of the merge phase",
            merge_duration_seconds.clone(),
        );

        let split_duration_seconds = Histogram::new(HISTOGRAM_BUCKETS);
        registry.register(
            "relativist_split_duration_seconds",
            "Duration of the split phase",
            split_duration_seconds.clone(),
        );

        let dispatch_bytes_total = Counter::default();
        registry.register(
            "relativist_dispatch_bytes_total",
            "Total bytes dispatched to workers",
            dispatch_bytes_total.clone(),
        );

        let return_bytes_total = Counter::default();
        registry.register(
            "relativist_return_bytes_total",
            "Total bytes received back from workers",
            return_bytes_total.clone(),
        );

        let interactions_by_rule_total = Family::<RuleLabel, Counter>::default();
        registry.register(
            "relativist_interactions_by_rule_total",
            "Total interactions by rule type across all workers and rounds",
            interactions_by_rule_total.clone(),
        );

        Self {
            rounds_total,
            round_duration_seconds,
            active_workers,
            partitions_dispatched_total,
            border_redexes,
            merge_duration_seconds,
            split_duration_seconds,
            dispatch_bytes_total,
            return_bytes_total,
            interactions_by_rule_total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_metrics() {
        let mut registry = Registry::default();
        let metrics = CoordinatorMetrics::register(&mut registry);

        // Verify counters start at 0
        assert_eq!(metrics.rounds_total.get(), 0);
        assert_eq!(metrics.partitions_dispatched_total.get(), 0);
        assert_eq!(metrics.dispatch_bytes_total.get(), 0);
        assert_eq!(metrics.return_bytes_total.get(), 0);

        // Verify gauges start at 0
        assert_eq!(metrics.active_workers.get(), 0);
        assert_eq!(metrics.border_redexes.get(), 0);
    }

    #[test]
    fn test_counter_increment() {
        let mut registry = Registry::default();
        let metrics = CoordinatorMetrics::register(&mut registry);

        metrics.rounds_total.inc();
        metrics.rounds_total.inc();
        assert_eq!(metrics.rounds_total.get(), 2);
    }

    #[test]
    fn test_gauge_set() {
        let mut registry = Registry::default();
        let metrics = CoordinatorMetrics::register(&mut registry);

        metrics.active_workers.set(4);
        assert_eq!(metrics.active_workers.get(), 4);

        metrics.active_workers.set(2);
        assert_eq!(metrics.active_workers.get(), 2);
    }

    #[test]
    fn test_histogram_observe() {
        let mut registry = Registry::default();
        let metrics = CoordinatorMetrics::register(&mut registry);

        // Observe a few values — should not panic
        metrics.round_duration_seconds.observe(0.005);
        metrics.round_duration_seconds.observe(0.1);
        metrics.round_duration_seconds.observe(2.5);
    }

    #[test]
    fn test_interactions_by_rule_total() {
        let mut registry = Registry::default();
        let metrics = CoordinatorMetrics::register(&mut registry);

        // Increment ConCon rule counter via get_or_create
        let label = RuleLabel {
            rule: RuleValue::ConCon,
        };
        metrics
            .interactions_by_rule_total
            .get_or_create(&label)
            .inc();
        metrics
            .interactions_by_rule_total
            .get_or_create(&label)
            .inc();

        // Increment EraEra rule counter
        let era_label = RuleLabel {
            rule: RuleValue::EraEra,
        };
        metrics
            .interactions_by_rule_total
            .get_or_create(&era_label)
            .inc();

        // Verify via encoding (Family counters are not directly readable)
        let mut buf = String::new();
        prometheus_client::encoding::text::encode(&mut buf, &registry).unwrap();
        assert!(buf.contains("relativist_interactions_by_rule_total"));
    }

    #[test]
    fn test_prometheus_encoding() {
        let mut registry = Registry::default();
        let metrics = CoordinatorMetrics::register(&mut registry);

        metrics.rounds_total.inc();
        metrics.active_workers.set(3);

        // Exercise interactions_by_rule_total so it appears in encoding
        let label = RuleLabel {
            rule: RuleValue::DupDup,
        };
        metrics
            .interactions_by_rule_total
            .get_or_create(&label)
            .inc();

        // Encode to OpenMetrics text format
        let mut buf = String::new();
        prometheus_client::encoding::text::encode(&mut buf, &registry).unwrap();

        assert!(buf.contains("relativist_rounds_total"));
        assert!(buf.contains("relativist_active_workers"));
        assert!(buf.contains("relativist_round_duration_seconds"));
        assert!(buf.contains("relativist_interactions_by_rule_total"));
    }
}
