//! Calendar version handling: contiguity checking (E11) and the covering-
//! version selector that rejects a window spanning a calendar change.
//!
//! Stage 2 works at operational-day granularity (dates). Resolving a phrase
//! like "yesterday" to shift-aligned timestamps is stage 4.

use chrono::NaiveDate;

use crate::config::CalendarVersionConfig;

/// Why a window could not resolve to a single calendar version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CalendarError {
    #[error("no calendar version covers {0}")]
    Uncovered(NaiveDate),
    #[error(
        "window spans a calendar change: {start} falls under '{start_version}', {end} under '{end_version}'"
    )]
    SpansChange {
        start: NaiveDate,
        end: NaiveDate,
        start_version: String,
        end_version: String,
    },
}

fn parse(d: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|e| format!("bad date '{d}': {e}"))
}

/// E11: versions of one family must tile the timeline with no overlap and no
/// gap. Only the last version (by start date) may be open-ended.
pub fn check_contiguous(versions: &[CalendarVersionConfig]) -> Result<(), String> {
    if versions.is_empty() {
        return Err("has no versions".into());
    }

    // Sort by effective_from without disturbing the caller's slice.
    let mut ordered: Vec<(&CalendarVersionConfig, NaiveDate)> = Vec::new();
    for v in versions {
        ordered.push((v, parse(&v.effective_from)?));
    }
    ordered.sort_by_key(|(_, from)| *from);

    for i in 0..ordered.len() {
        let (v, from) = &ordered[i];
        let is_last = i + 1 == ordered.len();
        match (&v.effective_until, is_last) {
            (None, true) => {} // the open-ended current version — fine
            (None, false) => {
                return Err(format!(
                    "version '{}' is open-ended but is not the latest",
                    v.version
                ));
            }
            (Some(until_str), _) => {
                let until = parse(until_str)?;
                if until < *from {
                    return Err(format!("version '{}' ends before it begins", v.version));
                }
                if !is_last {
                    let (next, next_from) = &ordered[i + 1];
                    let day_after = until.succ_opt().ok_or("date overflow")?;
                    if day_after > *next_from {
                        return Err(format!(
                            "versions '{}' and '{}' overlap",
                            v.version, next.version
                        ));
                    }
                    if day_after < *next_from {
                        return Err(format!(
                            "gap between versions '{}' and '{}'",
                            v.version, next.version
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// The version covering a single operational day, or `Uncovered`.
pub fn covering(
    versions: &[CalendarVersionConfig],
    day: NaiveDate,
) -> Result<&CalendarVersionConfig, CalendarError> {
    for v in versions {
        let from = parse(&v.effective_from).map_err(|_| CalendarError::Uncovered(day))?;
        let until = match &v.effective_until {
            Some(u) => parse(u).map_err(|_| CalendarError::Uncovered(day))?,
            None => NaiveDate::MAX,
        };
        if day >= from && day <= until {
            return Ok(v);
        }
    }
    Err(CalendarError::Uncovered(day))
}

/// The single version covering a `[start, end]` day range. Errors if the range
/// spans a calendar change — a comparison across a shift-pattern change is not
/// a comparison (`REGISTRY.md`, Sites And Calendars).
pub fn covering_range(
    versions: &[CalendarVersionConfig],
    start: NaiveDate,
    end: NaiveDate,
) -> Result<&CalendarVersionConfig, CalendarError> {
    let start_v = covering(versions, start)?;
    let end_v = covering(versions, end)?;
    if start_v.version != end_v.version {
        return Err(CalendarError::SpansChange {
            start,
            end,
            start_version: start_v.version.clone(),
            end_version: end_v.version.clone(),
        });
    }
    Ok(start_v)
}
