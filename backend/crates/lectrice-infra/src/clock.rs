use chrono::{SubsecRound, Utc};
use lectrice_domain::{Clock, Timestamp};

/// Wall-clock time, truncated to whole seconds so stored dates read `"2026-10-21T18:04:00Z"`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Utc::now().trunc_subsecs(0)
    }
}
