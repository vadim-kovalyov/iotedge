use crate::core::Request;

/// Trait to extend `Policy` engine core resource matching.
pub trait ResourceMatcher {
    /// This method is being called by `Policy` when it tries to match a `Request` to
    /// a resource in the policy rules.
    fn do_match<T>(&self, context: &Request<T>, input: &str, policy: &str) -> bool;
}

#[derive(Debug)]
pub struct DefaultResourceMatcher;

impl ResourceMatcher for DefaultResourceMatcher {
    fn do_match<T>(&self, _context: &Request<T>, input: &str, policy: &str) -> bool {
        input == policy
    }
}
