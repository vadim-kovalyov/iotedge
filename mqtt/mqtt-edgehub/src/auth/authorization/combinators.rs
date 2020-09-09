use mqtt_broker::auth::{Activity, Authorization, Authorizer};

pub struct AndThenAuthorizer<A1, A2>
where
    A1: Authorizer,
    A2: Authorizer,
{
    this: A1,
    next: A2,
}

impl<A1, A2> AndThenAuthorizer<A1, A2>
where
    A1: Authorizer,
    A2: Authorizer,
{
    pub fn new(this: A1, next: A2) -> Self {
        Self { this, next }
    }
}

impl<A1, A2, E> Authorizer for AndThenAuthorizer<A1, A2>
where
    A1: Authorizer<Error = E>,
    A2: Authorizer<Error = E>,
    E: std::error::Error,
{
    type Error = E;

    fn authorize(&self, activity: Activity) -> Result<Authorization, Self::Error> {
        if let result @ Ok(Authorization::Allowed) = self.this.authorize(activity.clone()) {
            result
        } else {
            self.next.authorize(activity)
        }
    }
}

// impl<T, A> T
// where
//     T: Authorizer,
//     A: Authorizer,
// {
//     pub fn and_then(&self, then: A) -> AndThenAuthorizer<T, A> {
//         AndThenAuthorizer::new(self, then)
//     }
// }
