use crate::{errors::Result, Error, PolicyDefinition, Statement};

/// Trait to extend `PolicyBuilder` validation for policy definition.
pub trait PolicyValidator {
    /// This method is being called by `PolicyBuilder` for policy definition
    /// while `Policy` is being constructed.
    ///
    /// If a policy definitions fails the validation, the error is returned.
    fn validate(&self, definition: &PolicyDefinition) -> Result<()>;
}

#[derive(Debug)]
pub struct DefaultValidator;

impl PolicyValidator for DefaultValidator {
    fn validate(&self, definition: &PolicyDefinition) -> Result<()> {
        let errors = definition
            .statements()
            .iter()
            .flat_map(|statement| visit_statement(statement))
            .collect::<Vec<_>>();

        if !errors.is_empty() {
            return Err(Error::ValidationSummary(errors));
        }
        Ok(())
    }
}

fn visit_statement(statement: &Statement) -> Vec<Error> {
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
    if statement.resources().is_empty() {
        result.push(Error::Validation("Resources list must not be empty".into()));
    }
    result
}
