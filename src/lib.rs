#![allow(dead_code)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
//#![deny(missing_docs)]

mod integration_test;
pub(crate) mod protocol_utils;
pub mod servers;

pub use servers::GenericServer;
pub use servers::MediaServer;
pub use servers::TextServer;
// pub use servers::RequestHandler;
// pub use servers::ServerType
