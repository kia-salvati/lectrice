use chrono::{DateTime, Utc};

/// UTC instant; serializes as ISO 8601 (`"2026-10-21T18:04:00Z"`).
pub type Timestamp = DateTime<Utc>;

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
