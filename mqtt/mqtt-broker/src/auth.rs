use std::error::Error as StdError;

use mqtt_broker_core::auth::{Authenticator, Authorizer};

pub fn authenticator() -> impl Authenticator<Error = Box<dyn StdError>> {
    imp::authenticator()
}

pub fn authorizer() -> impl Authorizer {
    imp::authorizer()
}

#[cfg(feature = "edgehub")]
mod imp {
    use std::error::Error as StdError;

    use mqtt_broker_core::auth::{authenticate_fn_ok, AuthId, Authenticator, MakeAuthorizer};
    use mqtt_opa_wasm::{MakeOpaAuthorizer, OpaAuthorizer};

    pub(super) fn authenticator() -> impl Authenticator<Error = Box<dyn StdError>> {
        authenticate_fn_ok(|_| Some(AuthId::Anonymous))
    }

    pub(super) fn authorizer() -> OpaAuthorizer {
        let wasm_bytes = opa_go::wasm::compile("data.edgehub.allow", "policy.rego").unwrap();

        MakeOpaAuthorizer::from_bytes(wasm_bytes)
            .unwrap()
            .make_authorizer()
            .unwrap()
    }
}

#[cfg(not(feature = "edgehub"))]
mod imp {
    use std::error::Error as StdError;

    use mqtt_broker_core::auth::{
        authenticate_fn_ok, authorize_fn_ok, AuthId, Authenticator, Authorization, Authorizer,
    };

    pub(super) fn authenticator() -> impl Authenticator<Error = Box<dyn StdError>> {
        authenticate_fn_ok(|_| Some(AuthId::Anonymous))
    }

    pub(super) fn authorizer() -> impl Authorizer {
        authorize_fn_ok(|_| Authorization::Allowed)
    }
}
