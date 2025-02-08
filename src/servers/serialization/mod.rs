use common::web_messages::{RequestMessage, Serializable, SerializationError};
use itertools::Chunk;
use itertools::{self, Itertools};
use wg_2024::packet::FRAGMENT_DSIZE;

/// testing module
#[cfg(test)]
mod test;

/// defragments and deserializes a request after all of its fragments
/// have been received
pub(super) fn defragment_deserialize_request(
    data: Vec<[u8; FRAGMENT_DSIZE]>,
) -> Result<RequestMessage, SerializationError> {
    RequestMessage::deserialize(data.into_flattened())
}

/// fragments a response into fragments
pub(super) fn fragment_response(data: Vec<u8>) -> Vec<[u8; FRAGMENT_DSIZE]> {
    data.into_iter()
        .chunks(FRAGMENT_DSIZE)
        .into_iter()
        .map(|c: Chunk<'_, std::vec::IntoIter<u8>>| {
            let mut v: Vec<u8> = c.collect::<Vec<u8>>();
            v.resize(FRAGMENT_DSIZE, 0);
            v.try_into().unwrap()
        })
        .collect()
}
