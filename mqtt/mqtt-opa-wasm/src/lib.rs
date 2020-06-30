use std::fmt;

use mqtt_broker_core::auth::{Activity, Authorization, Authorizer, MakeAuthorizer};
use opa_wasm::Policy;
use tracing::warn;

#[derive(Debug)]
pub enum Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error")
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

pub struct MakeOpaAuthorizer {
    module: Vec<u8>,
}

impl MakeOpaAuthorizer {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<MakeOpaAuthorizer, Error> {
        let auth = Self { module: bytes };
        Ok(auth)
    }
}

impl MakeAuthorizer for MakeOpaAuthorizer {
    type Authorizer = OpaAuthorizer;
    type Error = Error;

    fn make_authorizer(self) -> Result<Self::Authorizer, Self::Error> {
        OpaAuthorizer::from_bytes(&self.module)
    }
}

pub struct OpaAuthorizer {
    policy: Policy,
}

impl OpaAuthorizer {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let policy = Policy::from_wasm(bytes).unwrap();
        let auth = Self { policy };
        Ok(auth)
    }
}

impl Authorizer for OpaAuthorizer {
    type Error = Error;

    fn authorize(&self, activity: Activity) -> Result<Authorization, Self::Error> {
        let value = self.policy.evaluate(&activity).unwrap();
        warn!("Authorization received: {:?}", value);
        match value.try_into_set() {
            Ok(set) if !set.is_empty() => Ok(Authorization::Allowed),
            Ok(_) => Ok(Authorization::Forbidden(
                "Authorization denied by policy".to_string(),
            )),
            Err(e) => Ok(Authorization::Forbidden(format!(
                "Unable to evaluate policy {:?}",
                e
            ))),
        }
    }
}

unsafe impl Send for OpaAuthorizer {}