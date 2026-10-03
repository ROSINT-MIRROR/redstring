use crate::error::Error;

use super::observation::{DatePrecision, DateVal};

pub fn fmt_civil_date(secs: i64, precision: DatePrecision) -> Result<String, Error> {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    Ok(match precision {
        DatePrecision::Day => format!("{y:04}-{m:02}-{d:02}"),
        DatePrecision::Month => format!("{y:04}-{m:02}"),
        DatePrecision::Year => format!("{y}"),
        DatePrecision::Decade => format!("{}s", y.div_euclid(10) * 10),
        DatePrecision::Century => format!("{} c.", ordinal((y - 1).div_euclid(100) + 1)),
        DatePrecision::Circa => format!("~{y}"),
    })
}

pub fn fmt_civil_range(start: Option<DateVal>, end: Option<DateVal>) -> Result<String, Error> {
    Ok(match (start, end) {
        (None, None) => "…".to_owned(),
        (s, e) => format!("{}–{}", range_bound(s)?, range_bound(e)?),
    })
}

fn range_bound(v: Option<DateVal>) -> Result<String, Error> {
    let Some(d) = v else {
        return Ok("…".to_owned());
    };
    fmt_civil_date(d.secs, d.precision)
}

pub fn days_from_civil(y: i64, m: i64, d: i64) -> Result<i64, Error> {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

pub fn civil_ymd(secs: i64) -> Result<(i64, i64, i64), Error> {
    Ok(civil_from_days(secs.div_euclid(86_400)))
}

pub fn fnv1a(s: &str) -> Result<u64, Error> {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    Ok(h)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn ordinal(n: i64) -> String {
    let in_teens = (11..=13).contains(&n.rem_euclid(100));
    let ones = n.rem_euclid(10);
    let suffix = if in_teens {
        "th"
    } else if ones == 1 {
        "st"
    } else if ones == 2 {
        "nd"
    } else if ones == 3 {
        "rd"
    } else {
        "th"
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinal_suffixes_follow_english_rules() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(4), "4th");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(13), "13th");
        assert_eq!(ordinal(14), "14th");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(ordinal(22), "22nd");
        assert_eq!(ordinal(23), "23rd");
    }

    #[test]
    fn fmt_civil_date_renders_ce_dates() {
        assert_eq!(fmt_civil_date(0, DatePrecision::Day).unwrap(), "1970-01-01");
        assert_eq!(fmt_civil_date(1_700_000_000, DatePrecision::Day).unwrap(), "2023-11-14");
        assert_eq!(fmt_civil_date(946_684_800, DatePrecision::Year).unwrap(), "2000");
        assert_eq!(fmt_civil_date(946_684_800, DatePrecision::Century).unwrap(), "20th c.");
    }

    #[test]
    fn fmt_civil_date_renders_pre_1970_without_leaking_the_raw_epoch() {
        let pre = fmt_civil_date(-2_208_988_800, DatePrecision::Day).unwrap();
        assert_eq!(pre, "1900-01-01");
        assert!(!pre.contains("-22") && !pre.contains("988"), "raw epoch leaked: {pre}");
        assert_eq!(fmt_civil_date(-2_208_988_800, DatePrecision::Year).unwrap(), "1900");
    }

    #[test]
    fn civil_round_trips_including_pre_year_one() {
        for (y, m, d) in [(-100, 6, 15), (1, 1, 1), (1969, 7, 20), (2020, 2, 29)] {
            let days = days_from_civil(y, m, d).unwrap();
            assert_eq!(civil_from_days(days), (y, m, d), "round trip {y}-{m}-{d}");
        }
    }

    #[test]
    fn days_from_civil_matches_known_year_anchor() {
        assert_eq!(days_from_civil(2012, 1, 1).unwrap() * 86_400, 1_325_376_000);
    }

    #[test]
    fn fnv1a_is_stable_and_distinguishes_inputs() {
        assert_eq!(fnv1a("jane_victim").unwrap(), fnv1a("jane_victim").unwrap());
        assert_ne!(fnv1a("a").unwrap(), fnv1a("b").unwrap());
    }

    fn dv(secs: i64, precision: DatePrecision) -> DateVal {
        DateVal { secs, precision }
    }

    #[test]
    fn fmt_civil_range_joins_bounds_and_handles_open_ends() {
        let y2012 = dv(1_325_376_000, DatePrecision::Year);
        let y2013 = dv(1_356_998_400, DatePrecision::Year);
        assert_eq!(fmt_civil_range(Some(y2012), Some(y2013)).unwrap(), "2012–2013");
        assert_eq!(fmt_civil_range(Some(y2012), None).unwrap(), "2012–…");
        assert_eq!(fmt_civil_range(None, Some(y2013)).unwrap(), "…–2013");
        assert_eq!(fmt_civil_range(None, None).unwrap(), "…");
    }
}
