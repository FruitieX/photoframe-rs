//! Prometheus metrics, served in the text exposition format at `GET /metrics`.
//!
//! An update job's outcome is recorded once, after the device push's own
//! retries, so `photoframe_update_consecutive_failures` counts failed jobs, not
//! failed HTTP attempts. Individual failed attempts are counted separately in
//! `photoframe_push_attempt_failures_total`.

use once_cell::sync::Lazy;
use prometheus::{Encoder, GaugeVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder};
use std::time::{SystemTime, UNIX_EPOCH};

/// Outcome of one frame update job (scheduled or manually triggered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateResult {
    Success,
    /// The image could not be delivered to the device, even after retries.
    PushFailed,
    /// No source produced an image for the frame's orientation.
    NoImage,
    /// Anything else, e.g. the image could not be decoded or processed.
    Error,
}

impl UpdateResult {
    pub const ALL: [UpdateResult; 4] = [
        UpdateResult::Success,
        UpdateResult::PushFailed,
        UpdateResult::NoImage,
        UpdateResult::Error,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            UpdateResult::Success => "success",
            UpdateResult::PushFailed => "push_failed",
            UpdateResult::NoImage => "no_image",
            UpdateResult::Error => "error",
        }
    }
}

/// Why an HTTP push to a device failed. Kept small to bound label cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushFailureReason {
    /// Could not connect (refused, unreachable, connect timeout).
    Connect,
    /// Connected, but no complete response within the request timeout.
    Timeout,
    /// The device answered with a non-2xx status.
    HttpStatus,
    /// Any other transport error, e.g. the connection was reset mid-request.
    Request,
}

impl PushFailureReason {
    pub fn as_str(self) -> &'static str {
        match self {
            PushFailureReason::Connect => "connect",
            PushFailureReason::Timeout => "timeout",
            PushFailureReason::HttpStatus => "http_status",
            PushFailureReason::Request => "request",
        }
    }

    pub fn from_reqwest(e: &reqwest::Error) -> Self {
        // Connect timeouts report both is_connect and is_timeout; the frame was
        // never reached, so they count as connect failures.
        if e.is_connect() {
            PushFailureReason::Connect
        } else if e.is_timeout() {
            PushFailureReason::Timeout
        } else {
            PushFailureReason::Request
        }
    }
}

struct Metrics {
    registry: Registry,
    update_attempts: IntCounterVec,
    update_consecutive_failures: IntGaugeVec,
    last_update_success: GaugeVec,
    push_failures: IntCounterVec,
    push_attempt_failures: IntCounterVec,
    last_push_success: GaugeVec,
    source_errors: IntCounterVec,
}

static METRICS: Lazy<Metrics> = Lazy::new(|| {
    let registry = Registry::new();
    let update_attempts = IntCounterVec::new(
        Opts::new(
            "photoframe_update_attempts_total",
            "Frame update jobs by outcome (counted once per job, after push retries).",
        ),
        &["frame", "result"],
    )
    .unwrap();
    let update_consecutive_failures = IntGaugeVec::new(
        Opts::new(
            "photoframe_update_consecutive_failures",
            "Update jobs that have failed in a row for the frame; 0 after a success.",
        ),
        &["frame"],
    )
    .unwrap();
    let last_update_success = GaugeVec::new(
        Opts::new(
            "photoframe_last_update_success_timestamp_seconds",
            "Unix time of the frame's last successful update job.",
        ),
        &["frame"],
    )
    .unwrap();
    let push_failures = IntCounterVec::new(
        Opts::new(
            "photoframe_push_failures_total",
            "Pushes to a frame that failed after all retries, by reason of the last attempt.",
        ),
        &["frame", "reason"],
    )
    .unwrap();
    let push_attempt_failures = IntCounterVec::new(
        Opts::new(
            "photoframe_push_attempt_failures_total",
            "Individual failed HTTP attempts to push to a frame, including ones later retried.",
        ),
        &["frame", "reason"],
    )
    .unwrap();
    let last_push_success = GaugeVec::new(
        Opts::new(
            "photoframe_last_push_success_timestamp_seconds",
            "Unix time of the last image the frame accepted.",
        ),
        &["frame"],
    )
    .unwrap();
    let source_errors = IntCounterVec::new(
        Opts::new(
            "photoframe_source_errors_total",
            "Errors listing or fetching images from a source.",
        ),
        &["source", "kind"],
    )
    .unwrap();
    for c in [
        Box::new(update_attempts.clone()) as Box<dyn prometheus::core::Collector>,
        Box::new(update_consecutive_failures.clone()),
        Box::new(last_update_success.clone()),
        Box::new(push_failures.clone()),
        Box::new(push_attempt_failures.clone()),
        Box::new(last_push_success.clone()),
        Box::new(source_errors.clone()),
    ] {
        registry.register(c).unwrap();
    }
    Metrics {
        registry,
        update_attempts,
        update_consecutive_failures,
        last_update_success,
        push_failures,
        push_attempt_failures,
        last_push_success,
        source_errors,
    }
});

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Create a frame's series at zero so rates and alerts see them before the
/// first update runs.
pub fn init_frame(frame: &str) {
    let m = &*METRICS;
    for r in UpdateResult::ALL {
        m.update_attempts.with_label_values(&[frame, r.as_str()]);
    }
    m.update_consecutive_failures.with_label_values(&[frame]);
}

/// Create a source's error series at zero.
pub fn init_source(source: &str) {
    for kind in ["search", "fetch"] {
        METRICS.source_errors.with_label_values(&[source, kind]);
    }
}

pub fn record_update(frame: &str, result: UpdateResult) {
    let m = &*METRICS;
    m.update_attempts
        .with_label_values(&[frame, result.as_str()])
        .inc();
    let consecutive = m.update_consecutive_failures.with_label_values(&[frame]);
    if result == UpdateResult::Success {
        consecutive.set(0);
        m.last_update_success
            .with_label_values(&[frame])
            .set(now_secs());
    } else {
        consecutive.inc();
    }
}

pub fn record_push_success(frame: &str) {
    METRICS
        .last_push_success
        .with_label_values(&[frame])
        .set(now_secs());
}

pub fn record_push_attempt_failure(frame: &str, reason: PushFailureReason) {
    METRICS
        .push_attempt_failures
        .with_label_values(&[frame, reason.as_str()])
        .inc();
}

pub fn record_push_failure(frame: &str, reason: PushFailureReason) {
    METRICS
        .push_failures
        .with_label_values(&[frame, reason.as_str()])
        .inc();
}

/// `kind` is "search" (listing the source) or "fetch" (loading one image).
pub fn record_source_error(source: &str, kind: &str) {
    METRICS
        .source_errors
        .with_label_values(&[source, kind])
        .inc();
}

/// Render all metrics in the Prometheus text exposition format.
pub fn render() -> String {
    let mut buf = Vec::new();
    TextEncoder::new()
        .encode(&METRICS.registry.gather(), &mut buf)
        .expect("encoding metrics");
    String::from_utf8(buf).expect("metrics are UTF-8")
}

#[cfg(test)]
pub(crate) fn push_attempt_failures(frame: &str, reason: PushFailureReason) -> u64 {
    METRICS
        .push_attempt_failures
        .with_label_values(&[frame, reason.as_str()])
        .get()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Metrics are process-global; each test uses its own frame/source ids.

    fn consecutive(frame: &str) -> i64 {
        METRICS
            .update_consecutive_failures
            .with_label_values(&[frame])
            .get()
    }

    fn attempts(frame: &str, result: UpdateResult) -> u64 {
        METRICS
            .update_attempts
            .with_label_values(&[frame, result.as_str()])
            .get()
    }

    #[test]
    fn consecutive_failures_count_up_and_reset_on_success() {
        let f = "test-consecutive";
        init_frame(f);
        assert_eq!(consecutive(f), 0);
        record_update(f, UpdateResult::PushFailed);
        record_update(f, UpdateResult::NoImage);
        assert_eq!(consecutive(f), 2);
        assert_eq!(attempts(f, UpdateResult::PushFailed), 1);
        assert_eq!(attempts(f, UpdateResult::NoImage), 1);
        let last = METRICS.last_update_success.with_label_values(&[f]).get();
        assert_eq!(last, 0.0);

        record_update(f, UpdateResult::Success);
        assert_eq!(consecutive(f), 0);
        assert_eq!(attempts(f, UpdateResult::Success), 1);
        assert!(METRICS.last_update_success.with_label_values(&[f]).get() > 0.0);

        record_update(f, UpdateResult::Error);
        assert_eq!(consecutive(f), 1);
    }

    #[test]
    fn render_exposes_initialised_series() {
        init_frame("test-render");
        init_source("test-src");
        record_push_failure("test-render", PushFailureReason::Timeout);
        record_source_error("test-src", "search");
        let out = render();
        for line in [
            r#"photoframe_update_attempts_total{frame="test-render",result="push_failed"} 0"#,
            r#"photoframe_update_consecutive_failures{frame="test-render"} 0"#,
            r#"photoframe_push_failures_total{frame="test-render",reason="timeout"} 1"#,
            r#"photoframe_source_errors_total{kind="search",source="test-src"} 1"#,
            r#"photoframe_source_errors_total{kind="fetch",source="test-src"} 0"#,
        ] {
            assert!(out.contains(line), "missing {line} in:\n{out}");
        }
        assert!(out.contains("# TYPE photoframe_update_consecutive_failures gauge"));
    }
}
