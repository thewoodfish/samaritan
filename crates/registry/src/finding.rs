//! Validation findings — the E- and W-codes from `REGISTRY.md`.
//!
//! An error means the registry does not load. A warning means it loads but
//! something is probably wrong (a coverage gap, an unreachable term).

use std::fmt;

/// Every validation code the loader can raise. The `str` form matches
/// `REGISTRY.md` exactly, so a failure is greppable back to the spec.
///
/// Stage 2 implements the registry-owned checks. The graph-owned codes (E08,
/// E18–E23, W07–W09) are raised by the graph crate in stage 3 and are absent
/// here by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    // Errors — the registry does not load.
    E01AnalyzerUnknownSubject,
    E02AnalyzerUnknownDomain,
    E03AnalyzerUnknownIntent,
    E04DuplicateAnalyzerName,
    E05AttributeUnknownEnumeration,
    E06RatioDefinitionUnknownObservable,
    E07DerivedObservableOrphan,
    E09ZoneMissingEntity,
    E10ZoneUnknownRole,
    E11CalendarOverlapOrGap,
    E12SiteUnknownCalendarFamily,
    E13StrategyMissingForIntent,
    E14BaselineUnknownIntent,
    E15ObservableUnitInconsistent,
    E16DurationNamedHoursOrMinutes,
    E17RatioPercentageUnit,
    // Warnings — the registry loads.
    W01DomainWithoutAnalyzer,
    W02UnreachableSubject,
    W03UnreachableObservable,
    W04UnusedEnumeration,
    W05MaxAnalyzersExceedsRegistered,
    W06RestrictedZoneWithoutList,
}

impl Code {
    /// The bare code, e.g. `"E01"`.
    pub fn as_str(self) -> &'static str {
        use Code::*;
        match self {
            E01AnalyzerUnknownSubject => "E01",
            E02AnalyzerUnknownDomain => "E02",
            E03AnalyzerUnknownIntent => "E03",
            E04DuplicateAnalyzerName => "E04",
            E05AttributeUnknownEnumeration => "E05",
            E06RatioDefinitionUnknownObservable => "E06",
            E07DerivedObservableOrphan => "E07",
            E09ZoneMissingEntity => "E09",
            E10ZoneUnknownRole => "E10",
            E11CalendarOverlapOrGap => "E11",
            E12SiteUnknownCalendarFamily => "E12",
            E13StrategyMissingForIntent => "E13",
            E14BaselineUnknownIntent => "E14",
            E15ObservableUnitInconsistent => "E15",
            E16DurationNamedHoursOrMinutes => "E16",
            E17RatioPercentageUnit => "E17",
            W01DomainWithoutAnalyzer => "W01",
            W02UnreachableSubject => "W02",
            W03UnreachableObservable => "W03",
            W04UnusedEnumeration => "W04",
            W05MaxAnalyzersExceedsRegistered => "W05",
            W06RestrictedZoneWithoutList => "W06",
        }
    }

    /// Whether this code blocks loading.
    pub fn is_error(self) -> bool {
        self.as_str().starts_with('E')
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// One validation finding: a code and a human-readable detail naming the
/// offending term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub code: Code,
    pub detail: String,
}

impl Finding {
    pub fn new(code: Code, detail: impl Into<String>) -> Self {
        Finding {
            code,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.detail)
    }
}
