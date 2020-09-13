use std::collections::HashSet;
use std::{iter::FromIterator, str::FromStr};

use lazy_static::lazy_static;

use policy::{Error, PolicyDefinition, PolicyValidator, Result, Statement};

use crate::substituter;
use mqtt_broker::TopicFilter;

/// MQTT-specific implementation of `PolicyValidator`. It checks the following rules:
/// * Valid schema version.
/// * Presence of all elements in the policy definition (identities, operations, resources)
/// * Valid list of operations: mqtt:connect, mqtt:publish, mqtt:subscribe.
/// * Valid topic filter structure.
/// * Valid variable names.
#[derive(Debug)]
pub struct MqttValidator;

impl PolicyValidator for MqttValidator {
    fn validate(&self, definition: &PolicyDefinition) -> Result<()> {
        match definition.schema_version().as_ref() {
            "2020-10-30" => self.visit_definition(definition),
            version => Err(Error::Validation(format!(
                "Unsupported schema version: {}",
                version
            ))),
        }
    }
}

impl MqttValidator {
    fn visit_definition(&self, definition: &PolicyDefinition) -> Result<()> {
        for statement in definition.statements() {
            let statement_errors = self.visit_statement(statement);
            let identity_errors = statement
                .identities()
                .iter()
                .filter_map(|i| self.visit_identity(i).err());
            let operation_errors = statement
                .resources()
                .iter()
                .filter_map(|o| self.visit_operation(o).err());
            let resource_errors = statement
                .operations()
                .iter()
                .filter_map(|r| self.visit_resource(r).err());

            let errors = statement_errors
                .into_iter()
                .chain(identity_errors)
                .chain(operation_errors)
                .chain(resource_errors)
                .collect::<Vec<_>>();

            if !errors.is_empty() {
                return Err(Error::ValidationSummary(errors));
            }
        }
        Ok(())
    }

    fn visit_statement(&self, statement: &Statement) -> Vec<Error> {
        let mut result = vec![];
        if statement.identities().is_empty() {
            result.push(Error::Validation(
                "Identities list must not be empty".into(),
            ));
        }
        if statement.operations().is_empty() {
            result.push(Error::Validation(
                "Operations list must not be empty".into(),
            ));
        }
        // resources list can be empty only for connect operation.
        if statement.resources().is_empty() && !is_connect_op(statement) {
            result.push(Error::Validation("Resources list must not be empty".into()));
        }
        result
    }

    fn visit_identity(&self, value: &str) -> Result<()> {
        if value.is_empty() {
            return Err(Error::Validation(
                "Identity name must not be empty string".into(),
            ));
        }
        if let Some(variable) = substituter::extract_variable(value) {
            if VALID_IDENTITY_VARIABLES.get(variable).is_none() {
                return Err(Error::Validation(format!(
                    "unknown identity variable name: {}",
                    variable
                )));
            }
        }
        Ok(())
    }

    fn visit_operation(&self, value: &str) -> Result<()> {
        match value {
            "mqtt:publish" | "mqtt:subscribe" | "mqtt:connect" => Ok(()),
            _ => Err(Error::Validation(format!(
                r#"Unknown mqtt operation: "{}". List of supported operations: mqtt:publish, mqtt:subscribe, mqtt:connect"#,
                value
            ))),
        }
    }

    fn visit_resource(&self, value: &str) -> Result<()> {
        if value.is_empty() {
            return Err(Error::Validation("resource must not be empty".into()));
        }
        if let Some(variable) = substituter::extract_variable(value) {
            if VALID_RESOURCE_VARIABLES.get(variable).is_none() {
                return Err(Error::Validation(format!(
                    "unknown resource variable name: {}",
                    variable
                )));
            }
        }
        if let Err(e) = TopicFilter::from_str(value) {
            return Err(Error::Validation(format!("{}", e)));
        }
        Ok(())
    }
}

fn is_connect_op(statement: &Statement) -> bool {
    statement.operations().len() == 1 && statement.operations()[0] == "mqtt:connect"
}

lazy_static! {
    static ref VALID_IDENTITY_VARIABLES: HashSet<String> = HashSet::from_iter(vec![
        "iot:identity".into(),
        "iot:device_id".into(),
        "iot:module_id".into(),
        "mqtt:client_id".into(),
    ]);
    static ref VALID_RESOURCE_VARIABLES: HashSet<String> = HashSet::from_iter(vec![
        "iot:identity".into(),
        "iot:device_id".into(),
        "iot:module_id".into(),
        "mqtt:client_id".into(),
        "mqtt:topic".into()
    ]);
}
