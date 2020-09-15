use mqtt_broker::auth::{Activity, Operation};
use policy::{Request, Result, Substituter};

/// MQTT-specific implementation of `Substituter`. It replaces MQTT and IoT Hub specific variables:
/// * iot:identity
/// * iot:device_id
/// * iot:module_id
/// * iot:client_id
/// * iot:topic
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
                "{{mqtt:client_id}}" => replace(value, variable, context.client_id().as_str()),
                "{{iot:identity}}" => replace(
                    value,
                    variable,
                    &context.client_info().auth_id().to_string(),
                ),
                "{{iot:device_id}}" => replace(
                    value,
                    variable,
                    &context.client_info().auth_id().to_string(),
                ),
                "{{iot:module_id}}" => replace(
                    value,
                    variable,
                    &context.client_info().auth_id().to_string(),
                ),
                "{{mqtt:topic}}" => replace_topic(value, variable, context),
                _ => value.to_string(),
            };
        }
    }
    value.to_string()
}

pub(super) fn extract_variable(value: &str) -> Option<&str> {
    if let Some(start) = value.find("{{") {
        if let Some(end) = value.find("}}") {
            return Some(&value[start..end + 2]);
        }
    }
    None
}

fn replace_topic(value: &str, variable: &str, context: &Activity) -> String {
    match context.operation() {
        Operation::Publish(publish) => replace(value, variable, publish.publication().topic_name()),
        Operation::Subscribe(subscribe) => replace(value, variable, subscribe.topic_filter()),
        _ => value.to_string(),
    }
}

fn replace(value: &str, variable: &str, substitution: &str) -> String {
    value.replace(variable, substitution)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn extract_variable_test() {
        assert_eq!(
            "{{var}}",
            extract_variable("hello {{var}} variable").unwrap()
        );
    }
}
