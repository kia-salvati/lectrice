use lectrice_domain::AppResult;

use crate::context::RequestContext;

/// The service shape from the rules: context first, args second, failures raised by error name.
/// `NAME` is logged as the `service` field.
pub trait UseCase {
    const NAME: &'static str;
    type Args;
    type Output;

    fn run(&self, ctx: &RequestContext, args: Self::Args) -> AppResult<Self::Output>;
}
