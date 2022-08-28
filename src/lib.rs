use chrono::{FixedOffset, Utc};

/// Gets current date on "%Y%m%d" format for Peru timezone
pub fn get_peru_date() -> String {
    Utc::now()
        .with_timezone(&FixedOffset::west(5 * 3600))
        .format("%Y%m%d")
        .to_string()
}
