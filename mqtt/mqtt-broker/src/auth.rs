use std::error::Error as StdError;

use mqtt_broker_core::auth::{Authenticator, Authorizer};

pub fn authenticator() -> impl Authenticator<Error = Box<dyn StdError>> {
    imp::authenticator()
}

pub fn authorizer() -> impl Authorizer {
    imp::authorizer()
}

mod imp {
    use std::error::Error as StdError;

    use tracing::info;

    use mqtt_broker_core::auth::{authenticate_fn_ok, AuthId, Authenticator, MakeAuthorizer};
    use mqtt_opa_wasm::{MakeOpaAuthorizer, OpaAuthorizer};

    pub(super) fn authenticator() -> impl Authenticator<Error = Box<dyn StdError>> {
        authenticate_fn_ok(|_| Some(AuthId::Anonymous))
    }

    pub(super) fn authorizer() -> OpaAuthorizer {
        let rego = "policy.rego";
        info!("loading OPA policy from {:?}", rego);
        let wasm_bytes = opa_go::cli::compile("data.edgehub.allow", rego).unwrap();
        //let wasm_bytes = opa_go::wasm::compile("data.edgehub.allow", rego).unwrap();

        MakeOpaAuthorizer::from_bytes(wasm_bytes)
            .unwrap()
            .make_authorizer()
            .unwrap()
    }
}
