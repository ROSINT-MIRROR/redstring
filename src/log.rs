use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use crate::error::Error;

pub static ENABLED: AtomicBool = AtomicBool::new(false);

static SEQ: AtomicU64 = AtomicU64::new(0);
static START: OnceLock<Instant> = OnceLock::new();
static STORE: OnceLock<Mutex<LogState>> = OnceLock::new();

const CAP: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct Record {
    pub seq: u64,
    pub elapsed_ms: u128,
    pub level: Level,
    pub target: String,
    pub msg: String,
}

pub struct LogState {
    pub records: VecDeque<Record>,
    pub cap: usize,
    pub show_debug: bool,
    pub show_info: bool,
    pub show_warn: bool,
    pub show_error: bool,
    pub selected: BTreeSet<u64>,
    pub autoscroll: bool,
    pub open: bool,
}

#[derive(Clone)]
pub struct Snapshot {
    pub records: Vec<Record>,
    pub show_debug: bool,
    pub show_info: bool,
    pub show_warn: bool,
    pub show_error: bool,
    pub selected: BTreeSet<u64>,
    pub autoscroll: bool,
    pub open: bool,
}

pub fn enable() -> Result<(), Error> {
    START.get_or_init(Instant::now);
    ENABLED.store(true, Ordering::Relaxed);
    Ok(())
}

pub fn record(level: Level, target: &str, msg: String) -> Result<(), Error> {
    if !ENABLED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let start = START.get_or_init(Instant::now);
    let elapsed_ms = start.elapsed().as_millis();
    let rec = Record {
        seq,
        elapsed_ms,
        level,
        target: target.to_owned(),
        msg,
    };
    let Some(mut st) = lock_state() else {
        return Ok(());
    };
    record_into(true, &mut st, rec);
    Ok(())
}

pub fn snapshot() -> Result<Snapshot, Error> {
    let Some(st) = lock_state() else {
        return Ok(empty_snapshot());
    };
    Ok(Snapshot {
        records: st.records.iter().cloned().collect(),
        show_debug: st.show_debug,
        show_info: st.show_info,
        show_warn: st.show_warn,
        show_error: st.show_error,
        selected: st.selected.clone(),
        autoscroll: st.autoscroll,
        open: st.open,
    })
}

pub fn visible(snap: &Snapshot) -> Result<Vec<Record>, Error> {
    let mut out = Vec::new();
    for r in &snap.records {
        if snap_shows(snap, r.level) {
            out.push(r.clone());
        }
    }
    Ok(out)
}

pub fn format_records(records: &[Record]) -> Result<String, Error> {
    let mut out = String::new();
    for r in records {
        out.push_str(&format!(
            "{:>6} {:>9}ms {:<5} {:<14} {}\n",
            r.seq,
            r.elapsed_ms,
            level_tag(r.level),
            r.target,
            r.msg,
        ));
    }
    Ok(out)
}

pub fn set_filter(level: Level, on: bool) -> Result<(), Error> {
    let Some(mut st) = lock_state() else {
        return Ok(());
    };
    match level {
        Level::Debug => st.show_debug = on,
        Level::Info => st.show_info = on,
        Level::Warn => st.show_warn = on,
        Level::Error => st.show_error = on,
    }
    Ok(())
}

pub fn set_open(on: bool) -> Result<(), Error> {
    let Some(mut st) = lock_state() else {
        return Ok(());
    };
    st.open = on;
    Ok(())
}

pub fn is_open() -> Result<bool, Error> {
    let Some(st) = lock_state() else {
        return Ok(false);
    };
    Ok(st.open)
}

pub fn set_autoscroll(on: bool) -> Result<(), Error> {
    let Some(mut st) = lock_state() else {
        return Ok(());
    };
    st.autoscroll = on;
    Ok(())
}

pub fn select(seq: u64, on: bool) -> Result<(), Error> {
    let Some(mut st) = lock_state() else {
        return Ok(());
    };
    if on {
        st.selected.insert(seq);
    } else {
        st.selected.remove(&seq);
    }
    Ok(())
}

pub fn clear_selection() -> Result<(), Error> {
    let Some(mut st) = lock_state() else {
        return Ok(());
    };
    st.selected.clear();
    Ok(())
}

fn lock_state() -> Option<MutexGuard<'static, LogState>> {
    let Ok(g) = store().lock() else {
        return None;
    };
    Some(g)
}

fn store() -> &'static Mutex<LogState> {
    STORE.get_or_init(|| Mutex::new(new_state()))
}

fn new_state() -> LogState {
    LogState {
        records: VecDeque::new(),
        cap: CAP,
        show_debug: true,
        show_info: true,
        show_warn: true,
        show_error: true,
        selected: BTreeSet::new(),
        autoscroll: true,
        open: true,
    }
}

fn empty_snapshot() -> Snapshot {
    Snapshot {
        records: Vec::new(),
        show_debug: true,
        show_info: true,
        show_warn: true,
        show_error: true,
        selected: BTreeSet::new(),
        autoscroll: true,
        open: false,
    }
}

fn record_into(enabled: bool, state: &mut LogState, rec: Record) {
    if !enabled {
        return;
    }
    push_record(state, rec);
}

fn push_record(state: &mut LogState, rec: Record) {
    state.records.push_back(rec);
    while state.records.len() > state.cap {
        state.records.pop_front();
    }
}

fn snap_shows(snap: &Snapshot, level: Level) -> bool {
    match level {
        Level::Debug => snap.show_debug,
        Level::Info => snap.show_info,
        Level::Warn => snap.show_warn,
        Level::Error => snap.show_error,
    }
}

fn level_tag(level: Level) -> &'static str {
    match level {
        Level::Debug => "DEBUG",
        Level::Info => "INFO",
        Level::Warn => "WARN",
        Level::Error => "ERROR",
    }
}

#[macro_export]
macro_rules! dlog_debug {
    ($target:expr, $($arg:tt)*) => {
        if $crate::log::ENABLED.load(::core::sync::atomic::Ordering::Relaxed) {
            ::core::mem::drop($crate::log::record(
                $crate::log::Level::Debug,
                $target,
                ::std::format!($($arg)*),
            ));
        }
    };
}

#[macro_export]
macro_rules! dlog_info {
    ($target:expr, $($arg:tt)*) => {
        if $crate::log::ENABLED.load(::core::sync::atomic::Ordering::Relaxed) {
            ::core::mem::drop($crate::log::record(
                $crate::log::Level::Info,
                $target,
                ::std::format!($($arg)*),
            ));
        }
    };
}

#[macro_export]
macro_rules! dlog_warn {
    ($target:expr, $($arg:tt)*) => {
        if $crate::log::ENABLED.load(::core::sync::atomic::Ordering::Relaxed) {
            ::core::mem::drop($crate::log::record(
                $crate::log::Level::Warn,
                $target,
                ::std::format!($($arg)*),
            ));
        }
    };
}

#[macro_export]
macro_rules! dlog_error {
    ($target:expr, $($arg:tt)*) => {
        if $crate::log::ENABLED.load(::core::sync::atomic::Ordering::Relaxed) {
            ::core::mem::drop($crate::log::record(
                $crate::log::Level::Error,
                $target,
                ::std::format!($($arg)*),
            ));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(seq: u64, level: Level) -> Record {
        Record {
            seq,
            elapsed_ms: seq as u128,
            level,
            target: "t".to_owned(),
            msg: format!("m{seq}"),
        }
    }

    #[test]
    fn ring_evicts_oldest_beyond_cap() {
        let mut st = new_state();
        st.cap = 3;
        for i in 0..5 {
            push_record(&mut st, rec(i, Level::Info));
        }
        assert_eq!(st.records.len(), 3);
        assert_eq!(st.records.front().map(|r| r.seq), Some(2));
        assert_eq!(st.records.back().map(|r| r.seq), Some(4));
    }

    #[test]
    fn ring_under_cap_keeps_everything() {
        let mut st = new_state();
        st.cap = 10;
        for i in 0..4 {
            push_record(&mut st, rec(i, Level::Debug));
        }
        assert_eq!(st.records.len(), 4);
        assert_eq!(st.records.front().map(|r| r.seq), Some(0));
    }

    #[test]
    fn record_into_disabled_is_noop() {
        let mut st = new_state();
        record_into(false, &mut st, rec(0, Level::Info));
        assert!(st.records.is_empty());
        record_into(true, &mut st, rec(1, Level::Info));
        assert_eq!(st.records.len(), 1);
    }

    #[test]
    fn visible_respects_level_filters() {
        let mut snap = empty_snapshot();
        snap.records = vec![
            rec(0, Level::Debug),
            rec(1, Level::Info),
            rec(2, Level::Warn),
            rec(3, Level::Error),
        ];
        snap.show_debug = true;
        snap.show_info = true;
        snap.show_warn = true;
        snap.show_error = true;
        assert_eq!(visible(&snap).unwrap().len(), 4);

        snap.show_debug = false;
        snap.show_info = false;
        let kept = visible(&snap).unwrap();
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].level, Level::Warn);
        assert_eq!(kept[1].level, Level::Error);
    }

    #[test]
    fn format_records_has_one_line_per_record() {
        let recs = vec![rec(7, Level::Warn), rec(8, Level::Error)];
        let text = format_records(&recs).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("WARN"));
        assert!(lines[0].contains('7'));
        assert!(lines[1].contains("ERROR"));
    }

    #[test]
    fn selection_toggles_membership() {
        let mut st = new_state();
        st.selected.insert(3);
        assert!(st.selected.contains(&3));
        st.selected.remove(&3);
        assert!(!st.selected.contains(&3));
    }
}
