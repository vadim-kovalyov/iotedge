use mqtt_broker::{
    auth::{Activity, Authorization, Authorizer, Operation},
    AuthId,
};
use mqtt_policy::{MqttSubstituter, MqttValidator, TopicFilterMatcher};
use policy::{Decision, Error, Policy, PolicyBuilder, Request};

pub struct PolicyAuthorizer {
    policy: Policy<TopicFilterMatcher, MqttSubstituter>,
}

impl PolicyAuthorizer {
    #[allow(dead_code)]
    pub fn new(definition: &str) -> Result<Self, Error> {
        let policy = PolicyBuilder::from_json(definition)
            .with_validator(MqttValidator)
            .with_matcher(TopicFilterMatcher)
            .with_substituter(MqttSubstituter)
            .with_default_decision(Decision::Denied)
            .build()?;

        Ok(Self { policy })
    }
}

impl Authorizer for PolicyAuthorizer {
    type Error = Error;

    fn authorize(&self, activity: Activity) -> Result<Authorization, Self::Error> {
        let request = Request::with_context(
            get_identity(&activity).to_string(), //TODO: see if we can avoid cloning here.
            get_operation(&activity).to_string(),
            get_resource(&activity).to_string(),
            activity,
        )?;

        Ok(match self.policy.evaluate(&request)? {
            Decision::Allowed => Authorization::Allowed,
            Decision::Denied => Authorization::Forbidden("Denied by policy".into()),
        })
    }
}

fn get_identity(activity: &Activity) -> &str {
    match activity.client_info().auth_id() {
        AuthId::Anonymous => "anonymous", //TODO: think about this one.
        AuthId::Identity(identity) => identity.as_str(),
    }
}

fn get_operation(activity: &Activity) -> &str {
    match activity.operation() {
        Operation::Connect(_) => "mqtt:connect",
        Operation::Publish(_) => "mqtt:publish",
        Operation::Subscribe(_) => "mqtt:subscribe",
    }
}

fn get_resource(activity: &Activity) -> &str {
    match activity.operation() {
        // this is intentional. mqtt:connect should have empty resource.
        Operation::Connect(_) => "",
        Operation::Publish(publish) => publish.publication().topic_name(),
        Operation::Subscribe(subscribe) => subscribe.topic_filter(),
    }
}
