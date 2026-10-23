//! Use cases. Each service takes a `RequestContext` first and its args second.

pub mod context;
pub mod documents;
pub mod exports;
pub mod speech;
pub mod use_case;

pub use context::{Cancellation, RequestContext};
pub use use_case::UseCase;
