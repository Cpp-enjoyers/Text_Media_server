/*!
 * # `CppEnjoyers` implementation of `TextServer` and `MediaServer`
 *
 * Supports compression of packets (if requested by client).
 * Available compressions are:
 * - Huffman
 * - LZW
 *
 * Implements ETX (expected transmission rate) estimation of
 * drones in the network using an exponentially weighted moving
 * average: ETX(n) = a * p(n) + b * ETX(n - 1)
 */
// TODO ticks?

#![allow(dead_code)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![deny(nonstandard_style)]
//#![deny(missing_docs)]

#[cfg(test)]
mod integration_test;
pub mod protocol_utils;
pub mod servers;

pub use servers::GenericServer;
pub use servers::MediaServer;
pub use servers::TextServer;
// pub use servers::RequestHandler;
// pub use servers::ServerType
