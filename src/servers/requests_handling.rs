use common::web_messages::Request;
use wg_2024::packet::FRAGMENT_DSIZE;

use super::{serialization::deserialize_request, GenericServer};

impl GenericServer {
    pub(crate) fn handle_request(data: Vec<[u8; FRAGMENT_DSIZE]>) {
        if let Ok(req) = deserialize_request(data) {
            match req.content {
                Request::Type => todo!(),
                Request::Text(tr) => todo!(),
                Request::Media(mr) => todo!(),
            }
        } else {
            todo!();
        }
    }
}
