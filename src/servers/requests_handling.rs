use common::web_messages::Request;
use wg_2024::packet::FRAGMENT_DSIZE;

use super::{serialization::deserialize_request, GenericServer, MAX_SESSION_ID};

impl GenericServer {
    fn generate_response_id(sid: u64, rid: u16) -> u64 {
        (sid << 16) | u64::from(rid)
    }

    pub(crate) fn handle_request(&mut self, rid: u16, data: Vec<[u8; FRAGMENT_DSIZE]>) {
        if let Ok(req) = deserialize_request(data) {
            match req.content {
                Request::Type => {
                    // TODO create response
                },
                Request::Text(tr) => todo!(),
                Request::Media(mr) => todo!(),
            }
            self.session_id = (self.session_id + 1) & MAX_SESSION_ID;
        } else {
            todo!()
        }
    }
}
