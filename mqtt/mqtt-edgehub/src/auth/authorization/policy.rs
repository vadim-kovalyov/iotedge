use mqtt_broker::auth::{Activity, Authorization, Authorizer};
use mqtt_policy::{MqttSubstituter, TopicFilterMatcher};
use policy::{Decision, Error, Policy, Request};

pub struct PolicyEngineAuthorizer {
    policy: Policy<TopicFilterMatcher, MqttSubstituter>,
}

impl Authorizer for PolicyEngineAuthorizer {
    type Error = Error;

    fn authorize(&self, activity: Activity) -> Result<Authorization, Self::Error> {
        let request = Request::with_context("identity", "operation", "resource", activity)?;

        // request.properties.insert(
        //     "iot:identity".to_string(),
        //     activity.client_info().auth_id().to_string(),
        // );
        // request.properties.insert(
        //     "mqtt:client_id".to_string(),
        //     activity.client_id().to_string(),
        // );
        // request.properties.insert(
        //     "iot:device_id".to_string(),
        //     activity.client_info().auth_id().to_string(),
        // );
        // request.properties.insert(
        //     "iot:module_id".to_string(),
        //     activity.client_info().auth_id().to_string(),
        // );
        // request.properties.insert(
        //     "mqtt:topic".to_string(),
        //     activity.client_info().auth_id().to_string(),
        // );

        Ok(match self.policy.evaluate(&request)? {
            Decision::Allowed => Authorization::Allowed,
            Decision::Denied => Authorization::Forbidden("Denied by policy".into()),
        })
    }
}
