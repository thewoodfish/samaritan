//! Enum ↔ registry-key mapping. The registry speaks in strings ("Explain",
//! "OperationalPerformance"); the schema speaks in enums. One place to cross.

use samaritan_schema::{DomainType, IntentType};

pub fn intent_key(i: IntentType) -> &'static str {
    match i {
        IntentType::Explain => "Explain",
        IntentType::Compare => "Compare",
        IntentType::Locate => "Locate",
        IntentType::Recommend => "Recommend",
        IntentType::Predict => "Predict",
        IntentType::Summarize => "Summarize",
    }
}

pub fn domain_key(d: DomainType) -> &'static str {
    match d {
        DomainType::OperationalPerformance => "OperationalPerformance",
        DomainType::Production => "Production",
        DomainType::Equipment => "Equipment",
        DomainType::MaterialFlow => "MaterialFlow",
        DomainType::Infrastructure => "Infrastructure",
        DomainType::Personnel => "Personnel",
        DomainType::Safety => "Safety",
        DomainType::Security => "Security",
        DomainType::Environment => "Environment",
        DomainType::Logistics => "Logistics",
    }
}

/// A model-reported intent name to the enum, or `None` if unrecognized.
pub fn parse_intent(s: &str) -> Option<IntentType> {
    Some(match s {
        "Explain" => IntentType::Explain,
        "Compare" => IntentType::Compare,
        "Locate" => IntentType::Locate,
        "Recommend" => IntentType::Recommend,
        "Predict" => IntentType::Predict,
        "Summarize" => IntentType::Summarize,
        _ => return None,
    })
}

/// A model-reported domain name to the enum, or `None` if unrecognized (a
/// hallucinated domain is dropped, never guessed at).
pub fn parse_domain(s: &str) -> Option<DomainType> {
    Some(match s {
        "OperationalPerformance" => DomainType::OperationalPerformance,
        "Production" => DomainType::Production,
        "Equipment" => DomainType::Equipment,
        "MaterialFlow" => DomainType::MaterialFlow,
        "Infrastructure" => DomainType::Infrastructure,
        "Personnel" => DomainType::Personnel,
        "Safety" => DomainType::Safety,
        "Security" => DomainType::Security,
        "Environment" => DomainType::Environment,
        "Logistics" => DomainType::Logistics,
        _ => return None,
    })
}
