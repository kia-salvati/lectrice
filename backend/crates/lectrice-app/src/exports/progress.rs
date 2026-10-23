//! Export progress math: the time-left estimate and the 5 % checkpoints (logged and saved). Pure.

use std::time::Duration;

const PERCENT: u64 = 100;
/// Progress is logged and saved each time it passes another step of this size (spec §9).
const CHECKPOINT_PERCENT: u64 = 5;

/// How fast this export turns text into audio, measured on synthesized sentences only. Cached and silent
/// sentences finish at once, so counting them made the estimate far too low early on (W: "under a minute
/// left" at 71/150, then 174 s in all). The rate is the total wall time spent on synthesized sentences over
/// their characters, which smooths out single slow or fast sentences.
#[derive(Debug, Clone, Copy, Default)]
pub struct SynthesisRate {
    chars: u64,
    seconds: f64,
}

impl SynthesisRate {
    /// One synthesized sentence: its spoken characters and the wall time it took to come out of the pipeline.
    pub fn record(&mut self, chars: u64, wall: Duration) {
        self.chars += chars;
        self.seconds += wall.as_secs_f64();
    }

    /// `remaining_chars` at the measured rate, rounded up. `None` until a sentence was synthesized; `0` when
    /// nothing is left. Remaining sentences that turn out to be cached make it a little high, never low.
    pub fn seconds_left(&self, remaining_chars: u64) -> Option<u32> {
        if remaining_chars == 0 {
            return Some(0);
        }
        if self.chars == 0 {
            return None;
        }
        let seconds = remaining_chars as f64 * self.seconds / self.chars as f64;
        Some(seconds.ceil().min(f64::from(u32::MAX)) as u32)
    }
}

/// How many whole checkpoints `done` of `total` has passed (0 to 20). An empty export is complete.
pub fn checkpoint(done: u32, total: u32) -> u64 {
    if total == 0 {
        return PERCENT / CHECKPOINT_PERCENT;
    }
    u64::from(done.min(total)) * PERCENT / u64::from(total) / CHECKPOINT_PERCENT
}

pub fn percent(done: u32, total: u32) -> u64 {
    if total == 0 {
        return PERCENT;
    }
    u64::from(done.min(total)) * PERCENT / u64::from(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left_is_the_remaining_text_at_the_synthesis_rate() {
        let mut rate = SynthesisRate::default();
        assert_eq!(rate.seconds_left(1000), None);
        rate.record(250, Duration::from_secs(10));
        assert_eq!(rate.seconds_left(750), Some(30));
        rate.record(50, Duration::from_secs(2));
        assert_eq!(rate.seconds_left(700), Some(28));
        assert_eq!(rate.seconds_left(583), Some(24), "23.32 s rounds up");
        assert_eq!(rate.seconds_left(0), Some(0));
        assert_eq!(SynthesisRate::default().seconds_left(0), Some(0));
    }

    /// W finding: after many cached sentences, the old chars-over-elapsed estimate said "under a minute"
    /// for a run that still had minutes to go. Only synthesized sentences set the rate now.
    #[test]
    fn cached_sentences_do_not_speed_up_the_estimate() {
        let mut rate = SynthesisRate::default();
        // 70 cached sentences took no measurable time and are not recorded; one synthesized sentence of
        // 100 chars took 2 s, so 8000 remaining chars need 160 s.
        rate.record(100, Duration::from_secs(2));
        assert_eq!(rate.seconds_left(8000), Some(160));
    }

    #[test]
    fn checkpoints_are_every_five_percent() {
        assert_eq!(checkpoint(0, 200), 0);
        assert_eq!(checkpoint(9, 200), 0);
        assert_eq!(checkpoint(10, 200), 1);
        assert_eq!(checkpoint(199, 200), 19);
        assert_eq!(checkpoint(200, 200), 20);
        assert_eq!(checkpoint(1, 3), 6, "33 % is 6 whole steps");
        assert_eq!(checkpoint(0, 0), 20);
        assert_eq!((percent(1, 3), percent(0, 0)), (33, 100));
    }
}
