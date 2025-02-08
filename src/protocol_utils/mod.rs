#[cfg(test)]
mod test;

/// bitmask for the session ids (sid)
pub(crate) const SID_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
/// bitmask for the request ids (rid)
pub(crate) const RID_MASK: u64 = 0x0000_0000_0000_FFFF;

/// Generates the response id from the sid and the rid as required
/// by the protocol.
///
/// In the protocol a reponse id is 64 bit where:
/// - the 16 least significant bits represent the request id
///     of the request associated with the response
/// - the other 48 are the current session id of the Server
///
/// For example:
/// ``` text
///     given rid = 0x9 (0b1001) and sid = 0xD (0b1101)
///     the resulting response id will be: 0x9000D (0b1001000000001101)
/// ``` 
/// Rust example:
/// ```
/// # use ap2024_unitn_cppenjoyers_webservers::protocol_utils::generate_response_id;
/// # fn main() {
/// let sid = 1;
/// let rid = 2;
/// assert!(generate_response_id(sid, rid) == (sid << 16) | u64::from(rid));
/// # }
/// ```
#[inline]
#[must_use]
pub fn generate_response_id(sid: u64, rid: u16) -> u64 {
    (sid << 16) | u64::from(rid)
}

/// Returns the next sid to be used by the server, due to how the protocol
/// works, the sid wraps around 48 bits.
///
/// This means that the sid can be implemented as a an increasing wrapping counter
#[inline]
#[must_use]
pub fn next_sid(sid: u64) -> u64 {
    (sid + 1) & SID_MASK
}

/// Given a response id extract the rid (i.e. the least significant 16 bits)
///
/// ```
/// # use ap2024_unitn_cppenjoyers_webservers::protocol_utils::get_rid;
/// # fn main() {
/// let response_id = 0x34FE_88A2;
/// assert!(get_rid(response_id) == 0x88A2)
/// # }
/// ```
#[inline]
#[must_use]
pub fn get_rid(sid: u64) -> u16 {
    // intentional, if masked by 48 it fits into 16
    u16::try_from(sid & RID_MASK).unwrap_or(0)
}
