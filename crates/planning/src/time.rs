//! Time resolution — the deterministic heart of Planning.
//!
//! A relative expression plus `asked_at` plus the site's shift calendar
//! resolves to an absolute [`TimeWindow`], once, and never again. Boundaries
//! are the operational-day start, **not** midnight, and the calendar version
//! in force *then* governs — not the one in force today (`PLANNING.md`,
//! Stage 5; `REGISTRY.md`, Sites And Calendars).

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use samaritan_registry::config::CalendarVersionConfig;
use samaritan_registry::{Registry, calendar};
use samaritan_schema::{Id, TimeWindow, Timestamp};

use crate::error::ResolveError;

/// A shift instance as an absolute UTC interval `[start, end)`.
type ShiftInstance = (DateTime<Utc>, DateTime<Utc>);

/// Everything the resolver needs from the site, gathered once.
struct SiteClock<'a> {
    tz: Tz,
    timezone_name: String,
    family: String,
    versions: &'a [CalendarVersionConfig],
}

impl<'a> SiteClock<'a> {
    fn load(reg: &'a Registry, site_id: &str) -> Result<SiteClock<'a>, ResolveError> {
        let site = reg
            .site(site_id)
            .ok_or_else(|| ResolveError::UnknownSite(site_id.to_owned()))?;
        let versions = reg
            .calendar_versions(&site.calendar_family)
            .ok_or_else(|| {
                ResolveError::UnknownCalendarFamily(site.id.clone(), site.calendar_family.clone())
            })?;
        let tz: Tz = site
            .timezone
            .parse()
            .map_err(|_| ResolveError::BadTimezone(site.timezone.clone()))?;
        Ok(SiteClock {
            tz,
            timezone_name: site.timezone.clone(),
            family: site.calendar_family.clone(),
            versions,
        })
    }

    /// The UTC instant at which the operational day *labelled* by `date` begins,
    /// using the calendar version covering that date.
    fn op_day_start(&self, date: NaiveDate) -> Result<DateTime<Utc>, ResolveError> {
        let version = calendar::covering(self.versions, date)
            .map_err(|_| ResolveError::Uncovered(date.to_string()))?;
        let start = parse_hhmm(&version.operational_day_starts)?;
        let naive = date.and_time(start);
        let local = self
            .tz
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| ResolveError::AmbiguousLocalTime(naive.to_string()))?;
        Ok(local.with_timezone(&Utc))
    }

    /// The operational day containing `asked_at`: its label date and its start.
    fn current_op_day(
        &self,
        asked_at: Timestamp,
    ) -> Result<(NaiveDate, DateTime<Utc>), ResolveError> {
        let local_date = asked_at.with_timezone(&self.tz).date_naive();
        let today_start = self.op_day_start(local_date)?;
        if asked_at >= today_start {
            Ok((local_date, today_start))
        } else {
            let prev = local_date - Duration::days(1);
            Ok((prev, self.op_day_start(prev)?))
        }
    }

    /// The calendar id recorded on a window: `family@version` for the version
    /// covering `date`.
    fn calendar_id(&self, date: NaiveDate) -> Id {
        let version = calendar::covering(self.versions, date)
            .map(|v| v.version.as_str())
            .unwrap_or("unknown");
        Id(format!("{}@{version}", self.family))
    }

    /// Every shift instance overlapping the days around `asked_at`, sorted by
    /// start. A night shift crossing midnight is one instance.
    fn shift_instances(&self, around: NaiveDate) -> Result<Vec<ShiftInstance>, ResolveError> {
        let mut out = Vec::new();
        for offset in -1..=1 {
            let date = around + Duration::days(offset);
            let version = calendar::covering(self.versions, date)
                .map_err(|_| ResolveError::Uncovered(date.to_string()))?;
            for shift in &version.shifts {
                let start_t = parse_hhmm(&shift.start)?;
                let naive = date.and_time(start_t);
                let local = self
                    .tz
                    .from_local_datetime(&naive)
                    .single()
                    .ok_or_else(|| ResolveError::AmbiguousLocalTime(naive.to_string()))?;
                let start = local.with_timezone(&Utc);
                let end = start + Duration::seconds(shift.duration as i64);
                out.push((start, end));
            }
        }
        out.sort_by_key(|(s, _)| *s);
        out.dedup();
        Ok(out)
    }
}

/// Resolve `expr` against `asked_at` for `site`.
pub fn resolve_time(
    reg: &Registry,
    site_id: &str,
    expr: &str,
    asked_at: Timestamp,
) -> Result<TimeWindow, ResolveError> {
    let clock = SiteClock::load(reg, site_id)?;
    let norm = expr.trim().to_lowercase();

    // Build a window from two UTC instants, tagging it with the covering
    // calendar for the *start*.
    let window = |start: DateTime<Utc>, end: DateTime<Utc>, clock: &SiteClock| {
        let start_date = start.with_timezone(&clock.tz).date_naive();
        TimeWindow {
            start,
            end,
            resolved_from: expr.trim().to_owned(),
            calendar: clock.calendar_id(start_date),
            timezone: clock.timezone_name.clone(),
        }
    };

    match norm.as_str() {
        "now" => Ok(window(asked_at, asked_at, &clock)),

        "today" => {
            let (_, start) = clock.current_op_day(asked_at)?;
            Ok(window(start, asked_at, &clock))
        }

        "yesterday" => {
            let (cur_date, cur_start) = clock.current_op_day(asked_at)?;
            let y_start = clock.op_day_start(cur_date - Duration::days(1))?;
            Ok(window(y_start, cur_start, &clock))
        }

        "this week" => {
            let (cur_date, cur_start) = clock.current_op_day(asked_at)?;
            let start = clock.op_day_start(cur_date - Duration::days(6))?;
            Ok(window(start, cur_start.max(asked_at), &clock))
        }

        "this shift" => {
            let local_date = asked_at.with_timezone(&clock.tz).date_naive();
            let instances = clock.shift_instances(local_date)?;
            let cur = instances
                .iter()
                .find(|(s, e)| *s <= asked_at && asked_at < *e)
                .ok_or_else(|| ResolveError::Unresolvable(expr.to_owned()))?;
            Ok(window(cur.0, asked_at, &clock))
        }

        "last shift" => {
            let local_date = asked_at.with_timezone(&clock.tz).date_naive();
            let instances = clock.shift_instances(local_date)?;
            let last = instances
                .iter()
                .filter(|(_, e)| *e <= asked_at)
                .max_by_key(|(_, e)| *e)
                .ok_or_else(|| ResolveError::Unresolvable(expr.to_owned()))?;
            Ok(window(last.0, last.1, &clock))
        }

        other => resolve_parametric(other, expr, asked_at, &clock, &window),
    }
}

/// A default baseline: `days` operational days immediately before
/// `window_start`, truncated at the covering calendar version's start so it
/// never silently reaches across a calendar change (`REGISTRY.md`, Baseline
/// Defaults). Recorded as `resolved_from: "default baseline"`.
pub fn baseline_window(
    reg: &Registry,
    site_id: &str,
    days: u32,
    window_start: Timestamp,
) -> Result<TimeWindow, ResolveError> {
    let clock = SiteClock::load(reg, site_id)?;
    let start_local_date = window_start.with_timezone(&clock.tz).date_naive();

    // The version governing the window bounds how far back a default may reach.
    let version = calendar::covering(clock.versions, start_local_date)
        .map_err(|_| ResolveError::Uncovered(start_local_date.to_string()))?;
    let floor = NaiveDate::parse_from_str(&version.effective_from, "%Y-%m-%d")
        .map_err(|_| ResolveError::Unresolvable(version.effective_from.clone()))?;

    let candidate = start_local_date - Duration::days(days as i64);
    let baseline_date = candidate.max(floor);
    let start = clock.op_day_start(baseline_date)?;

    Ok(TimeWindow {
        start,
        end: window_start,
        resolved_from: "default baseline".to_owned(),
        calendar: clock.calendar_id(baseline_date),
        timezone: clock.timezone_name.clone(),
    })
}

/// Expressions carrying a number or a date: "last N days/shifts/hours",
/// "<date>", "<date> to <date>".
fn resolve_parametric(
    norm: &str,
    expr: &str,
    asked_at: Timestamp,
    clock: &SiteClock,
    window: &impl Fn(DateTime<Utc>, DateTime<Utc>, &SiteClock) -> TimeWindow,
) -> Result<TimeWindow, ResolveError> {
    // last N days / shifts / hours
    if let Some(rest) = norm.strip_prefix("last ") {
        let mut parts = rest.split_whitespace();
        if let (Some(num), Some(unit)) = (parts.next(), parts.next())
            && let Ok(n) = num.parse::<i64>()
        {
            let (_, cur_start) = clock.current_op_day(asked_at)?;
            match unit {
                "days" | "day" => {
                    let cur_date = cur_start.with_timezone(&clock.tz).date_naive();
                    let start = clock.op_day_start(cur_date - Duration::days(n))?;
                    return Ok(window(start, cur_start, clock));
                }
                "hours" | "hour" => {
                    let start = asked_at - Duration::hours(n);
                    return Ok(window(start, asked_at, clock));
                }
                "shifts" | "shift" => {
                    let local_date = asked_at.with_timezone(&clock.tz).date_naive();
                    let instances = clock.shift_instances(local_date)?;
                    let completed: Vec<_> =
                        instances.iter().filter(|(_, e)| *e <= asked_at).collect();
                    if let Some(nth) = completed.iter().rev().nth((n - 1) as usize) {
                        let end = completed.last().unwrap().1;
                        return Ok(window(nth.0, end, clock));
                    }
                }
                _ => {}
            }
        }
    }

    // <date> to <date>
    if let Some((a, b)) = norm.split_once(" to ")
        && let (Ok(d1), Ok(d2)) = (parse_date(a.trim()), parse_date(b.trim()))
    {
        // Reject a span that crosses a calendar change.
        calendar::covering_range(clock.versions, d1, d2)
            .map_err(|e| ResolveError::SpansCalendarChange(e.to_string()))?;
        let start = clock.op_day_start(d1)?;
        let end = clock.op_day_start(d2 + Duration::days(1))?;
        return Ok(window(start, end, clock));
    }

    // <date>
    if let Ok(d) = parse_date(norm) {
        let start = clock.op_day_start(d)?;
        let end = clock.op_day_start(d + Duration::days(1))?;
        return Ok(window(start, end, clock));
    }

    Err(ResolveError::Unresolvable(expr.to_owned()))
}

fn parse_hhmm(s: &str) -> Result<NaiveTime, ResolveError> {
    NaiveTime::parse_from_str(s, "%H:%M").map_err(|_| ResolveError::Unresolvable(s.to_owned()))
}

fn parse_date(s: &str) -> Result<NaiveDate, ()> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| ())
}
