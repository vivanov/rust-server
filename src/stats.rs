use std::convert::TryFrom;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::time::MissedTickBehavior;

use crate::shutdown::ShutdownHandle;

#[derive(Clone)]
pub struct Stats(Arc<Inner>);

struct Inner {
    shutdown_signal: ShutdownHandle,
    /// Total numer of calls into the MSE.
    calls_count: AtomicUsize,
    /// Total numer of compelted calls into the MSE.
    calls_completed_count: AtomicUsize,
    milliseconds_waited: AtomicUsize,
    summary: Arc<Mutex<Summary>>,
}

struct Summary {
    start: Instant,
    /// Highest TPS count of calls into the MSE.
    tps_highest: f64,
    /// Average TPS count of calls into the MSE.
    tps_average: f64,
    /// Average time spent waiting for MSE calls to complete.
    wait_time_average: Duration,
    /// Total time spent waiting for MSE calls to complete.
    wait_time_total: Duration,
    /// Highest pending gRPC requests.
    pending_highest: usize,
    /// Currently pending gRPC requests.
    pending_current: usize,
}

impl Stats {
    pub fn new(shutdown_signal: ShutdownHandle) -> Self {
        Stats(Arc::new(Inner {
            shutdown_signal,
            calls_count: AtomicUsize::new(0),
            calls_completed_count: AtomicUsize::new(0),
            milliseconds_waited: AtomicUsize::new(0),
            summary: Arc::new(Mutex::new(Summary {
                start: Instant::now(),
                tps_highest: 0.0,
                tps_average: 0.0,
                wait_time_average: Duration::ZERO,
                wait_time_total: Duration::ZERO,
                pending_highest: 0,
                pending_current: 0,
            })),
        }))
    }

    pub fn track_pending_call(&self) {
        self.0.calls_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn track_completed_call(&self, duration: Duration) {
        self.0.calls_completed_count.fetch_add(1, Ordering::Relaxed);
        self.0.milliseconds_waited.fetch_add(
            usize::try_from(duration.as_millis()).unwrap_or(usize::MAX),
            Ordering::Relaxed,
        );
    }

    pub async fn run_in_background(self) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut last_logged = Instant::now();
        let log_interval = Duration::from_secs(60);
        let mut shutdown_signal = self.0.shutdown_signal.signal();

        loop {
            let calls_count_before = self.0.calls_count.load(Ordering::Relaxed);
            let start = Instant::now();

            tokio::select! {
                _ = &mut shutdown_signal => {
                    break
                }
                _ = interval.tick() => {}
            };

            let mut summary = self.0.summary.lock().await;
            let elapsed = start.elapsed().as_secs_f64();

            // update highest TPS
            let calls_count_after = self.0.calls_count.load(Ordering::Relaxed);
            let calls_count =
                u32::try_from(calls_count_after - calls_count_before).unwrap_or(u32::MAX);
            let tps = if elapsed > 0.0 {
                f64::from(calls_count) / elapsed
            } else {
                0.0
            };
            if tps > summary.tps_highest {
                summary.tps_highest = tps;
            }

            // update average TPS
            let elapsed_total = summary.start.elapsed().as_secs_f64();
            let calls_count_total = u32::try_from(calls_count_after).unwrap_or(u32::MAX);
            summary.tps_average = if elapsed_total > 0.0 {
                f64::from(calls_count_total) / elapsed_total
            } else {
                0.0
            };

            // update time spent waiting for MSE calls to complete
            summary.wait_time_total = Duration::from_millis(
                u64::try_from(self.0.milliseconds_waited.load(Ordering::Relaxed))
                    .unwrap_or(u64::MAX),
            );
            summary.wait_time_average = summary
                .wait_time_total
                .checked_div(calls_count_total)
                .unwrap_or_default();

            // update pending requests
            summary.pending_current =
                calls_count_after - self.0.calls_completed_count.load(Ordering::Relaxed);
            if summary.pending_current > summary.pending_highest {
                summary.pending_highest = summary.pending_current;
            }

            // log summary every minute
            if last_logged.elapsed() > log_interval {
                last_logged = Instant::now();

                log::info!(
                    "avg. TPS = {:.2} | max. TPS = {:.2} | avg. wait time = {} | \
                    total wait time = {} | pending requests = {} | \
                    max. pending requests = {}",
                    summary.tps_average,
                    summary.tps_highest,
                    format_duration(summary.wait_time_average),
                    format_duration(summary.wait_time_total),
                    summary.pending_current,
                    summary.pending_highest,
                )
            }
        }
    }
}

fn format_duration(duration: Duration) -> String {
    if duration > Duration::from_secs(60) {
        format!("{:.2}min", duration.as_secs_f64() / 60.0)
    } else if duration > Duration::from_secs(1) {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        format!("{:.2}ms", duration.as_millis())
    }
}
