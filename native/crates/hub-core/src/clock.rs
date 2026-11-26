//! RFC 3339 timestamps in UTC, in the exact shape the shipped build writes.
//!
//! `Utc::now().to_rfc3339()` on the other side produces
//! `2026-11-25T08:31:00.123456789+00:00`: nine fractional digits and a `+00:00`
//! offset rather than `Z`. The note list sorts on `updated_at` **as text**, so the
//! width matters more than the notation -- a row that dropped its fraction would
//! sort before every row that kept one within the same second, and the list would
//! quietly reorder itself depending on which build wrote last.
//!
//! Written out rather than pulled from a date library: the whole requirement is one
//! format, and the conversion is Hinnant's civil-from-days with tests against known
//! epochs. A dependency that ships a timezone database to format `+00:00` is a
//! dependency nobody asked for.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time, formatted like the other build's `updated_at`.
pub fn now_iso() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    iso_from_unix(now.as_secs() as i64, now.subsec_nanos())
}

/// `2026-11-25T08:31:00.123456789+00:00` for a Unix timestamp.
pub fn iso_from_unix(seconds: i64, nanos: u32) -> String {
    // `div_euclid`/`rem_euclid`, not `/` and `%`: a pre-1970 timestamp must round
    // towards negative infinity or the day and the time-of-day disagree.
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = of_day / 3_600;
    let minute = (of_day % 3_600) / 60;
    let second = of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}+00:00")
}

/// Days since 1970-01-01 to a civil date. Howard Hinnant's `civil_from_days`.
///
/// Shifted epoch (`719_468`), 400-year eras, and the March-based year so the leap
/// day lands at the end -- the three tricks that make this branch-free and exact
/// for the whole proleptic Gregorian range.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = (z - era * 146_097) as u64; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_matches_the_shape_the_other_build_writes() {
        assert_eq!(
            iso_from_unix(0, 0),
            "1970-01-01T00:00:00.000000000+00:00"
        );
        // Width is the assertion: nine digits, `+00:00`, no `Z`.
        assert_eq!(iso_from_unix(0, 1).len(), "1970-01-01T00:00:00.000000000+00:00".len());
    }

    #[test]
    fn known_instants_round_trip_through_the_era_arithmetic() {
        // 2023-11-14T22:13:20Z, the instant `Date.now()` examples use.
        assert_eq!(iso_from_unix(1_700_000_000, 0), "2023-11-14T22:13:20.000000000+00:00");
        // 2025-01-01T00:00:00Z
        assert_eq!(iso_from_unix(1_735_689_600, 0), "2025-01-01T00:00:00.000000000+00:00");
        // 2024-02-29T00:00:00Z: the leap day is where a naive /365 goes wrong.
        assert_eq!(iso_from_unix(1_709_164_800, 0), "2024-02-29T00:00:00.000000000+00:00");
    }

    #[test]
    fn timestamps_sort_lexicographically_in_time_order() {
        // The list is `ORDER BY updated_at DESC` on text, so this is the property
        // that actually matters: the format must be monotonic as a string.
        let earlier = iso_from_unix(1_700_000_000, 999_999_999);
        let later = iso_from_unix(1_700_000_001, 0);
        assert!(earlier < later, "{earlier} must sort before {later}");
    }

    #[test]
    fn a_pre_epoch_instant_keeps_the_day_and_clock_consistent() {
        // -1 is 1969-12-31T23:59:59Z. With `/` and `%` it would format as
        // 1970-01-01T-1:59:59 and break both the sort and the date.
        assert_eq!(iso_from_unix(-1, 0), "1969-12-31T23:59:59.000000000+00:00");
    }
}
