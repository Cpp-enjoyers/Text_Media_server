/*!
 * # `CppEnjoyers` implementation of [`TextServer`] and [`MediaServer`]
 *
 * Supports compression of packets (if requested by client).
 * Available compressions are:
 * - Huffman
 * - LZW
 *
 * Uses the log crate to trace network operations and debug, to abilitate
 * the logs, simply set the environment variable `RUST_LOG` to the desired
 * level (`info`, `warn`, `error`).
 *
 * The [`GenericServer`] uses ETX estimation to decide the best routing paths.
 *
 * The estimator uses an exponentially weighted moving average (EWMA),
 * the formula is as follows:
 * ``` text
 *     ETX(n) = p(n) * alpha + ETX(n - 1) * beta
 *     ETX(0) = default_etx_value
 * ```
 * where:
 * - ETX(n) is the ETX at time n
 * - p(n) is the estimated ETX at time n, calculated from the last k samples (k is a predefined constant)
 * - alpha and beta are parameters that decide how fast the ETX adapts to change
 *
 * # Simulation controller interaction
 * The [`GenericServer`] can accept different command by the scl:
 * - `AddSender(ID, Channel)`: adds a new direct neighbor to the server
 * - `RemoveSender(ID)`: removes a direct neighbor from the server
 * - `Shortcut(Packet)`: delivers to the server a packet that has been shortcutted
 *
 * The [`GenericServer`] can send different events to the scl:
 * - `PacketSent(Packet)`: logs that a packet has been sent over the network
 * - `Shortcut(Packet)`: sends a packet that generated an error but cannot be dropped
 *
 * # High level protocol
 *
 * The protocol between Client and Server is defined as follows:
 * - Upon discorvering a Server node the Client can query it to discover its type
 * - The server answers with a response containing its type
 * - After that the Client can query the Server for a list of available file or for a specific file
 * - The Server answer with the requested information or with an error in case of unknown/unsupported requests
 *
 * Every request is associated with a request id (16 bits) that will be part of the response id used by the server.
 * In this way the Client can easily recognise the request associated with the response and handle it accordingly.
 *
 * Every request/response is serialized and fragmented into binary before being sent as packets in the netowork.
 * Optionally the Client can specify in the request a compression method to use on the serialized data: this
 * can help reduce the network bottleneck due to less packets being sent.
 */

#![allow(dead_code)]
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![deny(nonstandard_style)]
#![warn(missing_docs)]

#[cfg(test)]
mod integration_test;
/// This module offers utilities to calculate session ids and request ids
/// as specified by the protocol implemented by Clients and Servers
pub mod protocol_utils;
/// This module contains the public API of [`GenericServer`], the struct used
/// to implement a server that can adhere to the used protocol and that can
/// be extended to handle the request in the necessary way according to
/// its [`servers::ServerType`]
pub mod servers;

#[doc(inline)]
pub use servers::GenericServer;
#[doc(inline)]
pub use servers::MediaServer;
#[doc(inline)]
pub use servers::TextServer;
// pub use servers::RequestHandler;
// pub use servers::ServerType
