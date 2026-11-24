//! Sample collection and the distribution report the gate reads.

use std::fmt::Write as _;

#[derive(Debug, Default)]
pub struct Samples {
    /// hotkey_down → window moved on screen. **The gate**; see `spike.rs`.
    pub shown_us: Vec<u64>,
    /// hotkey_down → the repaint for this reveal finished. Reported, not gated.
    pub refresh_us: Vec<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Distribution {
    pub n: usize,
    pub min: u64,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
}

impl Distribution {
    pub fn as_ms(self) -> [f64; 5] {
        [
            self.min as f64 / 1000.0,
            self.p50 as f64 / 1000.0,
            self.p95 as f64 / 1000.0,
            self.p99 as f64 / 1000.0,
            self.max as f64 / 1000.0,
        ]
    }
}

/// Nearest-rank percentile on an ascending slice.
///
/// Nearest-rank (not interpolation) on purpose: the gate says "p95 ≤ 60 ms", and with
/// interpolation a run of 200 could report 59.9 while the 190th sample is actually
/// 61. Nearest-rank always returns a sample that really happened.
pub fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[index]
}

pub fn distribution(values: &[u64]) -> Distribution {
    if values.is_empty() {
        return Distribution {
            n: 0,
            min: 0,
            p50: 0,
            p95: 0,
            p99: 0,
            max: 0,
        };
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Distribution {
        n: sorted.len(),
        min: sorted[0],
        p50: percentile(&sorted, 50.0),
        p95: percentile(&sorted, 95.0),
        p99: percentile(&sorted, 99.0),
        max: *sorted.last().unwrap(),
    }
}

impl Samples {
    pub fn push(&mut self, shown_us: u64, refresh_us: u64) {
        self.shown_us.push(shown_us);
        self.refresh_us.push(refresh_us);
    }
}

/// Human-readable report; also the exact text the measurement wrapper greps.
pub fn report(samples: &Samples, budget_p95_ms: f64, budget_p99_ms: f64) -> String {
    let shown = distribution(&samples.shown_us);
    let refresh = distribution(&samples.refresh_us);
    let [smin, sp50, sp95, sp99, smax] = shown.as_ms();
    let [rmin, rp50, rp95, rp99, rmax] = refresh.as_ms();

    let mut out = String::new();
    let _ = writeln!(out, "native spike latency");
    let _ = writeln!(out, "  samples={} (nearest-rank percentiles)", shown.n);
    let _ = writeln!(
        out,
        "  shown   ms (gate): min={smin:.1} p50={sp50:.1} p95={sp95:.1} p99={sp99:.1} max={smax:.1}"
    );
    let _ = writeln!(
        out,
        "  refresh ms (content): min={rmin:.1} p50={rp50:.1} p95={rp95:.1} p99={rp99:.1} max={rmax:.1}"
    );

    let p95_ok = sp95 <= budget_p95_ms;
    let p99_ok = sp99 <= budget_p99_ms;
    let _ = writeln!(
        out,
        "  budget: p95 <= {budget_p95_ms:.0} ms ({}) p99 <= {budget_p99_ms:.0} ms ({})",
        if p95_ok { "PASS" } else { "FAIL" },
        if p99_ok { "PASS" } else { "FAIL" }
    );
    let _ = writeln!(
        out,
        "  headroom: p95 {:+.1} ms, p99 {:+.1} ms",
        budget_p95_ms - sp95,
        budget_p99_ms - sp99
    );
    out
}

pub fn report_json(
    samples: &Samples,
    budget_p95_ms: f64,
    budget_p99_ms: f64,
    probe: Option<crate::probe_json::ProbeJson>,
) -> String {
    let shown = distribution(&samples.shown_us);
    let refresh = distribution(&samples.refresh_us);
    let [smin, sp50, sp95, sp99, smax] = shown.as_ms();
    let [rmin, rp50, rp95, rp99, rmax] = refresh.as_ms();

    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"kind\": \"native-s1-latency\",");
    let _ = writeln!(out, "  \"n\": {},", shown.n);
    let _ = writeln!(
        out,
        "  \"shown\": {{ \"min\": {smin:.1}, \"p50\": {sp50:.1}, \"p95\": {sp95:.1}, \"p99\": {sp99:.1}, \"max\": {smax:.1} }},"
    );
    let _ = writeln!(
        out,
        "  \"refresh\": {{ \"min\": {rmin:.1}, \"p50\": {rp50:.1}, \"p95\": {rp95:.1}, \"p99\": {rp99:.1}, \"max\": {rmax:.1} }},"
    );
    let _ = writeln!(
        out,
        "  \"budget\": {{ \"p95_ms\": {budget_p95_ms:.0}, \"p99_ms\": {budget_p99_ms:.0}, \"p95_ok\": {}, \"p99_ok\": {} }},",
        sp95 <= budget_p95_ms,
        sp99 <= budget_p99_ms
    );
    match probe {
        Some(probe) => {
            let _ = writeln!(
                out,
                "  \"probe\": {{ \"private_mb\": {:.1}, \"working_set_mb\": {:.1}, \"handles\": {}, \"cpu_ms\": {:.0}, \"idle_cpu_ms\": {:.0} }}",
                probe.private_mb,
                probe.working_set_mb,
                probe.handles,
                probe.cpu_ms,
                probe.idle_cpu_ms
            );
        }
        None => {
            let _ = writeln!(out, "  \"probe\": null");
        }
    }
    let _ = writeln!(out, "}}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_never_invents_a_value() {
        let values: Vec<u64> = (1..=200).collect();
        assert_eq!(percentile(&values, 95.0), 190);
        assert_eq!(percentile(&values, 50.0), 100);
        assert_eq!(percentile(&values, 99.0), 198);
    }

    #[test]
    fn single_sample_reports_itself_everywhere() {
        let d = distribution(&[42]);
        assert_eq!((d.n, d.p50, d.p95, d.p99, d.max), (1, 42, 42, 42, 42));
    }

    #[test]
    fn empty_distribution_is_zero_not_a_panic() {
        let d = distribution(&[]);
        assert_eq!(d.n, 0);
        assert_eq!(d.p95, 0);
    }

    #[test]
    fn report_flags_a_budget_failure() {
        let mut samples = Samples::default();
        for _ in 0..10 {
            samples.push(70_000, 80_000);
        }
        let text = report(&samples, 60.0, 100.0);
        assert!(text.contains("p95 <= 60 ms (FAIL)"));
        assert!(text.contains("samples=10"));
    }

    #[test]
    fn json_is_parseable_and_records_the_budget_verdict() {
        let mut samples = Samples::default();
        for _ in 0..4 {
            samples.push(30_000, 40_000);
        }
        let json = report_json(&samples, 60.0, 100.0, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["n"], 4);
        assert_eq!(parsed["budget"]["p95_ok"], true);
        assert_eq!(parsed["shown"]["p95"], 30.0);
    }
}
