//! The combined-speed history behind the Downloads screen's throughput
//! bars: the last [`SAMPLES`] samples, at most one a second, in memory.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Bars drawn, and samples kept.
pub(crate) const SAMPLES: usize = 20;

/// Combined speed samples, oldest first. Fed from job progress events, so
/// nothing samples (or wakes the app) while no download runs.
#[derive(Default)]
pub(crate) struct SpeedHistory {
    samples: VecDeque<u64>,
    last: Option<Instant>,
}

impl SpeedHistory {
    /// Adds `speed` (bytes a second) unless a sample was taken less than a
    /// second before `now`. A zero after a non-zero sample is always added,
    /// so the bars fall to zero when every job stalls: nothing may sample
    /// again for a long while.
    pub(crate) fn record(&mut self, now: Instant, speed: u64) {
        let falls_to_zero = speed == 0 && self.samples.back().is_some_and(|&last| last > 0);
        if !falls_to_zero
            && self
                .last
                .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(1))
        {
            return;
        }
        self.last = Some(now);
        if self.samples.len() == SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(speed);
    }

    /// Oldest first.
    pub(crate) fn samples(&self) -> impl Iterator<Item = u64> + '_ {
        self.samples.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn keeps_the_last_twenty_samples_one_a_second() {
        let start = Instant::now();
        let mut history = SpeedHistory::default();
        for n in 0..25u64 {
            history.record(start + Duration::from_secs(n), n);
            // A second event within the same second adds nothing.
            history.record(
                start + Duration::from_secs(n) + Duration::from_millis(400),
                999,
            );
        }
        let kept: Vec<u64> = history.samples().collect();
        assert_eq!(
            kept,
            (5..25).collect::<Vec<_>>(),
            "the oldest samples drop off, oldest first"
        );
    }

    #[core::prelude::v1::test]
    fn a_zero_after_a_speed_is_always_added() {
        let start = Instant::now();
        let mut history = SpeedHistory::default();
        history.record(start, 5);
        history.record(start + Duration::from_millis(200), 0);
        history.record(start + Duration::from_millis(400), 0);
        let kept: Vec<u64> = history.samples().collect();
        assert_eq!(
            kept,
            [5, 0],
            "a stall drops the bars to zero at once, and only once"
        );
    }
}
