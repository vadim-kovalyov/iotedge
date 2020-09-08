use std::str::FromStr;

use mqtt_broker::TopicFilter;
use policy::{Field, PolicyValidator, Request, ResourceMatcher, Result, Substituter};

pub struct TopicFilterMatcher;

impl ResourceMatcher for TopicFilterMatcher {
    fn do_match(&self, _context: &Request, input: &str, policy: &str) -> bool {
        if let Ok(filter) = TopicFilter::from_str(policy) {
            filter.matches(input)
        } else {
            false
        }
    }
}

pub struct MqttSubstituter;

impl Substituter for MqttSubstituter {
    fn visit_identity(&self, value: &str, context: &Request) -> Result<String> {
        Ok(replace_variable(value, context))
    }

    fn visit_resource(&self, value: &str, context: &Request) -> Result<String> {
        Ok(replace_variable(value, context))
    }
}

fn replace_variable(value: &str, context: &Request) -> String {
    if let Some(variable) = extract_variable(value) {
        if let Some(substitution) = context.properties.get(variable) {
            return value.replace(&format!("{{{}}}", variable), substitution);
        }
    }
    value.to_string()
}

fn extract_variable(value: &str) -> Option<&str> {
    if let Some(start) = value.find("{{") {
        if let Some(end) = value.find("}}") {
            return Some(&value[start..end]);
        }
    }
    None
}

pub struct MqttValidator;

impl PolicyValidator for MqttValidator {
    fn validate(&self, _field: Field, _value: &str) -> Result<()> {
        Ok(())
    }
}
