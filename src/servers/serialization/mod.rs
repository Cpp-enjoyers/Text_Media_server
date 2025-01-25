use common::web_messages::{RequestMessage, ResponseMessage, Serializable, SerializationError};
use itertools::Chunk;
use itertools::{self, Itertools};
use wg_2024::packet::FRAGMENT_DSIZE;

#[cfg(test)]
mod test;

pub(crate) fn deserialize_request(
    data: Vec<[u8; FRAGMENT_DSIZE]>,
) -> Result<RequestMessage, SerializationError> {
    RequestMessage::deserialize(data.into_iter().flatten().collect())
}

pub(crate) fn serialize_response(
    data: &ResponseMessage,
) -> Result<Vec<[u8; FRAGMENT_DSIZE]>, SerializationError> {
    Ok(ResponseMessage::serialize(data)?
        .into_iter()
        .chunks(128)
        .into_iter()
        .map(|c: Chunk<'_, std::vec::IntoIter<u8>>| c.collect::<Vec<u8>>().try_into().unwrap())
        .collect())
}
