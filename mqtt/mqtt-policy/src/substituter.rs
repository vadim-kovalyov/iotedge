use mqtt_broker::auth::Activity;
use policy::{Request, Result, Substituter};

#[derive(Debug)]
pub struct MqttSubstituter;

impl Substituter for MqttSubstituter {
    type Context = Activity;

    fn visit_identity(&self, value: &str, context: &Request<Self::Context>) -> Result<String> {
        Ok(replace_variable(value, context))
    }

    fn visit_resource(&self, value: &str, context: &Request<Self::Context>) -> Result<String> {
        Ok(replace_variable(value, context))
    }
}

fn replace_variable(value: &str, context: &Request<Activity>) -> String {
    if let Some(context) = context.context() {
        if let Some(variable) = extract_variable(value) {
            return match variable {
                "iot:identity" => replace(
                    value,
                    variable,
                    &context.client_info().auth_id().to_string(),
                ),
                _ => value.to_string(),
            };
        }
    }
    value.to_string()
}

pub(super) fn extract_variable(value: &str) -> Option<&str> {
    if let Some(start) = value.find("{{") {
        if let Some(end) = value.find("}}") {
            return Some(&value[start..end]);
        }
    }
    None
}

fn replace(value: &str, variable: &str, substitution: &str) -> String {
    value.replace(&format!("{{{}}}", variable), substitution)
}
