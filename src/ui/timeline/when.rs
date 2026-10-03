use crate::error::Error;
use crate::model::{
    DatePrecision, DateVal, civil_ymd, days_from_civil, fmt_civil_date, fmt_civil_range,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Utc,
    OffsetMinutes(i64),
    Unstated,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum When {
    Date(DateVal),
    Between(DateVal, DateVal),
    From(DateVal),
    Until(DateVal),
    Instant { secs: i64, zone: Zone },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bounds {
    pub start: i64,
    pub end: i64,
    pub open_start: bool,
    pub open_end: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Reading {
    Dated(When),
    NotADate,
}

impl When {
    pub const DAY: i64 = 86_400;

    pub fn bounds(&self) -> Result<Bounds, Error> {
        Ok(match self {
            When::Date(dv) => {
                let (start, end) = period(*dv)?;
                Bounds {
                    start,
                    end,
                    open_start: false,
                    open_end: false,
                }
            }
            When::Between(a, b) => Bounds {
                start: period(*a)?.0,
                end: period(*b)?.1,
                open_start: false,
                open_end: false,
            },
            When::From(a) => {
                let (start, end) = period(*a)?;
                Bounds {
                    start,
                    end,
                    open_start: false,
                    open_end: true,
                }
            }
            When::Until(b) => {
                let (start, end) = period(*b)?;
                Bounds {
                    start,
                    end,
                    open_start: true,
                    open_end: false,
                }
            }
            When::Instant { secs, zone } => instant_bounds(*secs, *zone),
        })
    }

    pub fn is_duration(&self) -> Result<bool, Error> {
        Ok(matches!(
            self,
            When::Between(..) | When::From(..) | When::Until(..)
        ))
    }

    pub fn label(&self) -> Result<String, Error> {
        match self {
            When::Date(dv) => fmt_civil_date(dv.secs, dv.precision),
            When::Between(a, b) => fmt_civil_range(Some(*a), Some(*b)),
            When::From(a) => fmt_civil_range(Some(*a), None),
            When::Until(b) => fmt_civil_range(None, Some(*b)),
            When::Instant { secs, zone } => instant_label(*secs, *zone),
        }
    }

    pub fn note(&self) -> Result<&'static str, Error> {
        Ok(match self {
            When::Date(dv) => precision_note(dv.precision),
            When::Between(..) => "a period — both ends stated",
            When::From(..) => "from this date — the end is not stated",
            When::Until(..) => "until this date — the start is not stated",
            When::Instant { zone, .. } => zone_note(*zone),
        })
    }
}

pub fn read_text_date(raw: &str) -> Result<Reading, Error> {
    let s = raw.trim();
    let Some(date) = s.get(..10) else {
        return Ok(Reading::NotADate);
    };
    let Some((y, m, d)) = ymd10(date) else {
        return Ok(Reading::NotADate);
    };
    let day = day_start(y, m, d)?;
    if civil_ymd(day)? != (y, m, d) {
        return Ok(Reading::NotADate);
    }
    let Some(rest) = s.get(10..) else {
        return Ok(Reading::NotADate);
    };
    if rest.is_empty() {
        let dv = DateVal {
            secs: day,
            precision: DatePrecision::Day,
        };
        return Ok(Reading::Dated(When::Date(dv)));
    }
    let Some(clock_and_zone) = strip_separator(rest) else {
        return Ok(Reading::NotADate);
    };
    let Some((clock, zone)) = split_zone(clock_and_zone) else {
        return Ok(Reading::NotADate);
    };
    let Some(sod) = seconds_of_day(clock) else {
        return Ok(Reading::NotADate);
    };
    let secs = day + sod - zone_offset_secs(zone);
    Ok(Reading::Dated(When::Instant { secs, zone }))
}

fn period(dv: DateVal) -> Result<(i64, i64), Error> {
    let (y, m, d) = civil_ymd(dv.secs)?;
    Ok(match dv.precision {
        DatePrecision::Day => {
            let start = day_start(y, m, d)?;
            (start, start + When::DAY)
        }
        DatePrecision::Month => {
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            (day_start(y, m, 1)?, day_start(ny, nm, 1)?)
        }
        DatePrecision::Year | DatePrecision::Circa => {
            (day_start(y, 1, 1)?, day_start(y + 1, 1, 1)?)
        }
        DatePrecision::Decade => {
            let y0 = y.div_euclid(10) * 10;
            (day_start(y0, 1, 1)?, day_start(y0 + 10, 1, 1)?)
        }
        DatePrecision::Century => {
            let c0 = (y - 1).div_euclid(100) * 100 + 1;
            (day_start(c0, 1, 1)?, day_start(c0 + 100, 1, 1)?)
        }
    })
}

fn day_start(y: i64, m: i64, d: i64) -> Result<i64, Error> {
    Ok(days_from_civil(y, m, d)? * When::DAY)
}

fn instant_bounds(secs: i64, zone: Zone) -> Bounds {
    if zone == Zone::Unstated {
        let start = secs.div_euclid(When::DAY) * When::DAY;
        return Bounds {
            start,
            end: start + When::DAY,
            open_start: false,
            open_end: false,
        };
    }
    Bounds {
        start: secs,
        end: secs,
        open_start: false,
        open_end: false,
    }
}

fn instant_label(secs: i64, zone: Zone) -> Result<String, Error> {
    let wall = secs + zone_offset_secs(zone);
    let (y, m, d) = civil_ymd(wall)?;
    let sod = wall.rem_euclid(When::DAY);
    let (h, mi) = (sod / 3600, (sod % 3600) / 60);
    Ok(format!(
        "{y:04}-{m:02}-{d:02} {h:02}:{mi:02} {}",
        zone_tag(zone)
    ))
}

fn zone_offset_secs(zone: Zone) -> i64 {
    match zone {
        Zone::OffsetMinutes(min) => min * 60,
        Zone::Utc | Zone::Unstated => 0,
    }
}

fn zone_tag(zone: Zone) -> String {
    match zone {
        Zone::Utc => "UTC".to_owned(),
        Zone::OffsetMinutes(min) => {
            let sign = if min < 0 { '-' } else { '+' };
            let abs = min.abs();
            format!("{sign}{:02}:{:02}", abs / 60, abs % 60)
        }
        Zone::Unstated => "(zone not stated)".to_owned(),
    }
}

fn precision_note(p: DatePrecision) -> &'static str {
    match p {
        DatePrecision::Day => "day stated — time of day not stated",
        DatePrecision::Month => "month stated — day not stated",
        DatePrecision::Year => "year stated — month and day not stated",
        DatePrecision::Decade => "decade only",
        DatePrecision::Century => "century only",
        DatePrecision::Circa => "approximate year (circa)",
    }
}

fn zone_note(zone: Zone) -> &'static str {
    match zone {
        Zone::Utc => "exact time, UTC",
        Zone::OffsetMinutes(..) => "exact time, with the UTC offset the record states",
        Zone::Unstated => "time of day stated without a zone — placed by its day",
    }
}

fn digits(s: &str) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    let mut n: i64 = 0;
    for c in s.chars() {
        let d = c.to_digit(10)?;
        n = n * 10 + i64::from(d);
    }
    Some(n)
}

fn ymd10(date: &str) -> Option<(i64, i64, i64)> {
    let mut parts = date.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d) = (digits(y)?, digits(m)?, digits(d)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

fn strip_separator(rest: &str) -> Option<&str> {
    let mut chars = rest.chars();
    let first = chars.next()?;
    if first == 'T' || first == 't' || first == ' ' {
        return Some(chars.as_str());
    }
    None
}

fn split_zone(t: &str) -> Option<(&str, Zone)> {
    let Some(at) = t.find(['Z', 'z', '+', '-']) else {
        return Some((t, Zone::Unstated));
    };
    let (clock, tail) = t.split_at(at);
    if tail.eq_ignore_ascii_case("z") {
        return Some((clock, Zone::Utc));
    }
    Some((clock, Zone::OffsetMinutes(offset_minutes(tail)?)))
}

fn offset_minutes(tail: &str) -> Option<i64> {
    let mut chars = tail.chars();
    let sign = chars.next()?;
    let body = chars.as_str().replace(':', "");
    if body.len() != 4 {
        return None;
    }
    let hh = digits(body.get(..2)?)?;
    let mm = digits(body.get(2..)?)?;
    if hh > 14 || mm > 59 {
        return None;
    }
    let total = hh * 60 + mm;
    if sign == '+' {
        return Some(total);
    }
    if sign == '-' {
        return Some(-total);
    }
    None
}

fn seconds_of_day(clock: &str) -> Option<i64> {
    let whole = clock.split('.').next()?;
    let mut total: i64 = 0;
    let mut count = 0usize;
    for part in whole.split(':') {
        if part.len() != 2 {
            return None;
        }
        let n = digits(part)?;
        let limit = if count == 0 { 23 } else { 59 };
        if n > limit {
            return None;
        }
        total = total * 60 + n;
        count += 1;
    }
    if count == 2 {
        return Some(total * 60);
    }
    if count == 3 {
        return Some(total);
    }
    None
}
