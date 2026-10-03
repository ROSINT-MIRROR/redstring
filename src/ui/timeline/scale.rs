use crate::error::Error;
use crate::model::{civil_ymd, days_from_civil};

use super::state::{TimelineState, View};
use super::when::When;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ruler {
    pub left: f32,
    pub width: f32,
    pub view: View,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Tick {
    pub x: f32,
    pub label: String,
    pub major: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Days(i32),
    Months(i32),
    Years(i32),
}

impl Step {
    const LADDER: [Step; 15] = [
        Step::Days(1),
        Step::Days(2),
        Step::Days(7),
        Step::Days(14),
        Step::Months(1),
        Step::Months(2),
        Step::Months(3),
        Step::Months(6),
        Step::Years(1),
        Step::Years(2),
        Step::Years(5),
        Step::Years(10),
        Step::Years(25),
        Step::Years(50),
        Step::Years(100),
    ];
    const MAX_TICKS: usize = 400;

    fn approx_secs(self) -> f64 {
        match self {
            Step::Days(n) => f64::from(n) * 86_400.0,
            Step::Months(n) => f64::from(n) * 2_629_746.0,
            Step::Years(n) => f64::from(n) * 31_556_952.0,
        }
    }
}

impl Ruler {
    pub const TICK_GAP_PX: f64 = 92.0;

    pub fn new(left: f32, width: f32, view: View) -> Result<Ruler, Error> {
        Ok(Ruler { left, width, view })
    }

    pub fn x(&self, secs: i64) -> Result<f32, Error> {
        Ok(self.left + px((num(secs)? - self.view.t0) / self.view.spp))
    }

    pub fn secs_at(&self, x: f32) -> Result<f64, Error> {
        Ok(self.view.t0 + f64::from(x - self.left) * self.view.spp)
    }

    pub fn spanning(lo: i64, hi: i64, width: f32) -> Result<View, Error> {
        let w = f64::from(width.max(1.0));
        let spp = (num(hi - lo)? / w).clamp(TimelineState::MIN_SPP, TimelineState::MAX_SPP);
        Ok(View { t0: num(lo)?, spp })
    }

    pub fn ticks(&self) -> Result<Vec<Tick>, Error> {
        let from = whole(self.view.t0);
        let to = whole(self.view.t0 + f64::from(self.width) * self.view.spp);
        let mut out = Vec::new();
        match self.step() {
            Step::Days(n) => self.day_ticks(from, to, i64::from(n), &mut out)?,
            Step::Months(n) => self.month_ticks(from, to, i64::from(n), &mut out)?,
            Step::Years(n) => self.year_ticks(from, to, i64::from(n), &mut out)?,
        }
        Ok(out)
    }

    fn step(&self) -> Step {
        for step in Step::LADDER {
            if step.approx_secs() / self.view.spp >= Ruler::TICK_GAP_PX {
                return step;
            }
        }
        Step::Years(100)
    }

    fn day_ticks(&self, from: i64, to: i64, n: i64, out: &mut Vec<Tick>) -> Result<(), Error> {
        let mut day = from.div_euclid(When::DAY).div_euclid(n) * n;
        while day * When::DAY <= to && out.len() < Step::MAX_TICKS {
            let secs = day * When::DAY;
            let ymd = civil_ymd(secs)?;
            let major = ymd.2 == 1;
            let label = if major || out.is_empty() {
                format!("{:04}-{:02}-{:02}", ymd.0, ymd.1, ymd.2)
            } else {
                format!("{:02}-{:02}", ymd.1, ymd.2)
            };
            self.push(secs, label, major, out)?;
            day += n;
        }
        Ok(())
    }

    fn month_ticks(&self, from: i64, to: i64, n: i64, out: &mut Vec<Tick>) -> Result<(), Error> {
        let ymd = civil_ymd(from)?;
        let mut year = ymd.0;
        let mut month = (ymd.1 - 1).div_euclid(n) * n + 1;
        while out.len() < Step::MAX_TICKS {
            let secs = days_from_civil(year, month, 1)? * When::DAY;
            if secs > to {
                break;
            }
            self.push(secs, format!("{year:04}-{month:02}"), month == 1, out)?;
            month += n;
            while month > 12 {
                month -= 12;
                year += 1;
            }
        }
        Ok(())
    }

    fn year_ticks(&self, from: i64, to: i64, n: i64, out: &mut Vec<Tick>) -> Result<(), Error> {
        let mut year = civil_ymd(from)?.0.div_euclid(n) * n;
        while out.len() < Step::MAX_TICKS {
            let secs = days_from_civil(year, 1, 1)? * When::DAY;
            if secs > to {
                break;
            }
            self.push(secs, format!("{year}"), year.rem_euclid(n * 5) == 0, out)?;
            year += n;
        }
        Ok(())
    }

    fn push(
        &self,
        secs: i64,
        label: String,
        major: bool,
        out: &mut Vec<Tick>,
    ) -> Result<(), Error> {
        let x = self.x(secs)?;
        if x < self.left - 1.0 {
            return Ok(());
        }
        out.push(Tick { x, label, major });
        Ok(())
    }
}

fn num(v: i64) -> Result<f64, Error> {
    let days = i32::try_from(v.div_euclid(When::DAY)).map_err(|source| Error::Timeline {
        what: format!("{v} s is beyond the calendar this timeline can draw: {source}"),
    })?;
    let rest = i32::try_from(v.rem_euclid(When::DAY)).map_err(|source| Error::Timeline {
        what: format!("seconds-of-day overflow for {v}: {source}"),
    })?;
    Ok(f64::from(days) * 86_400.0 + f64::from(rest))
}

fn px(v: f64) -> f32 {
    v as f32
}

fn whole(v: f64) -> i64 {
    v.floor() as i64
}
