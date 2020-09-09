use std::str::FromStr;

use mqtt_broker::TopicFilter;
use policy::{Field, PolicyValidator, Request, ResourceMatcher, Result};

mod substituter;

pub use crate::substituter::MqttSubstituter;

pub struct TopicFilterMatcher;

impl ResourceMatcher for TopicFilterMatcher {
    fn do_match<Activity>(&self, _context: &Request<Activity>, input: &str, policy: &str) -> bool {
        if let Ok(filter) = TopicFilter::from_str(policy) {
            filter.matches(input)
        } else {
            false
        }
    }
}

pub struct MqttValidator;

impl PolicyValidator for MqttValidator {
    fn validate(&self, _field: Field, _value: &str) -> Result<()> {
        Ok(())
    }
}
