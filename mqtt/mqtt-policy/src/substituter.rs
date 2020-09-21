use mqtt_broker::auth::{Activity, Operation};
use policy::{Request, Result, Substituter};

/// MQTT-specific implementation of `Substituter`. It replaces MQTT and IoT Hub specific variables:
/// * iot:identity
/// * iot:device_id
/// * iot:module_id
/// * iot:client_id
/// * iot:topic
#[derive(Debug)]
pub struct MqttSubstituter {
    device_id: String,
}

impl MqttSubstituter {
    pub fn new(device_id: impl Into<String>) -> Self {
        Self {
            device_id: device_id.into(),
        }
    }

    fn device_id(&self) -> &str {
        &self.device_id
    }

    fn replace_variable(&self, value: &str, context: &Request<Activity>) -> String {
        if let Some(context) = context.context() {
            if let Some(variable) = extract_variable(value) {
                return match variable {
                    "{{mqtt:client_id}}" => replace(value, variable, context.client_id().as_str()),
                    "{{iot:identity}}" => replace(
                        value,
                        variable,
                        &context.client_info().auth_id().to_string(),
                    ),
                    "{{iot:device_id}}" => replace(value, variable, &extract_device_id(&context)),
                    "{{iot:module_id}}" => replace(value, variable, &extract_module_id(&context)),
                    "{{iot:this_device_id}}" => replace(value, variable, self.device_id()),
                    "{{mqtt:topic}}" => replace_topic(value, variable, context),
                    _ => value.to_string(),
                };
            }
        }
        value.to_string()
    }
}

impl Substituter for MqttSubstituter {
    type Context = Activity;

    fn visit_identity(&self, value: &str, context: &Request<Self::Context>) -> Result<String> {
        Ok(self.replace_variable(value, context))
    }

    fn visit_resource(&self, value: &str, context: &Request<Self::Context>) -> Result<String> {
        Ok(self.replace_variable(value, context))
    }
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

fn extract_device_id(activity: &Activity) -> String {
    let auth_id = activity.client_info().auth_id().to_string();
    auth_id
        .split('/')
        .next()
        .map(str::to_owned)
        .unwrap_or_default()
}

fn extract_module_id(activity: &Activity) -> String {
    let auth_id = activity.client_info().auth_id().to_string();
    auth_id
        .split('/')
        .nth(1)
        .map(str::to_owned)
        .unwrap_or_default()
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
