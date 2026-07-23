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
