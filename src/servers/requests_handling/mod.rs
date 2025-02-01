use std::{
    fs::{self, read},
    io,
};

use common::{
    slc_commands::{ServerEvent, ServerType},
    web_messages::{Compression, Request, ResponseMessage, Serializable, TextRequest},
};
use compression::{bypass::BypassCompressor, lzw::LZWCompressor, Compressor};
use log::{error, info, warn};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Fragment, Packet, FRAGMENT_DSIZE},
};

use super::{
    serialization::{defragment_deserialize_request, fragment_response},
    GenericServer, SID_MASK,
};

use crate::servers::ServerType as ST;

#[cfg(test)]
mod test;

fn list_dir(path: &str) -> Result<Vec<String>, io::Error> {
    Ok(fs::read_dir(path)?
        .filter(Result::is_ok)
        .map(|p| p.unwrap().path())
        .filter(|p| p.is_file())
        .map(|p| p.into_os_string().into_string().unwrap())
        .collect())
}

#[inline]
fn generate_response_id(sid: u64, rid: u16) -> u64 {
    (sid << 16) | u64::from(rid)
}

impl<T: ST> GenericServer<T> {
    fn compress(data: Vec<u8>, comp: &Compression) -> Result<Vec<u8>, String> {
        match comp {
            Compression::LZW => LZWCompressor::new()
                .compress(data)?
                .serialize()
                .map_err(|_| "Error during compression".to_string()),
            Compression::None => BypassCompressor::new().compress(data),
        }
    }

    pub(crate) fn handle_request(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
        rid: u16,
        data: Vec<[u8; FRAGMENT_DSIZE]>,
    ) {
        if let Ok(req) = defragment_deserialize_request(data) {
            let resp: ResponseMessage;
            #[allow(clippy::match_wildcard_for_single_variants)]
            match req.content {
                Request::Type => {
                    resp = ResponseMessage::new_type_response(
                        self.id,
                        req.compression_type,
                        ServerType::FileServer,
                    );
                }
                Request::Text(tr) => match tr {
                    TextRequest::TextList => {
                        resp = ResponseMessage::new_text_list_response(
                            self.id,
                            req.compression_type,
                            list_dir("./public/").unwrap_or_default(),
                        );
                    }
                    TextRequest::Text(str) => {
                        resp = if let Ok(data) = read(str) {
                            ResponseMessage::new_text_response(self.id, req.compression_type, data)
                        } else {
                            ResponseMessage::new_not_found_response(self.id, req.compression_type)
                        }
                    }
                },
                _ => {
                    resp = ResponseMessage::new_invalid_request_response(
                        self.id,
                        req.compression_type,
                    );
                }
            }
            info!(target: &self.target_topic, "Sending response: {resp:?}");
            self.send_response(srch, src_id, rid, &resp);
        } else {
            error!(target: &self.target_topic, "Received undeserializable request, sending invalid response");
        }
        // self.session_id = (self.session_id + 1) & SID_MASK;
    }

    pub(crate) fn send_response(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
        rid: u16,
        resp: &ResponseMessage,
    ) {
        let mut resp_hdr: SourceRoutingHeader = self.get_routing_hdr_with_hint(srch, src_id);

        if resp_hdr.len() < 2 {
            error!(target: &self.target_topic, "Error, srch of response inconsistent: {resp_hdr}. Dropping response");
            return;
        }

        resp_hdr.increase_hop_index();
        let serialized: Result<Vec<[u8; FRAGMENT_DSIZE]>, String>;
        if let Ok(data) = resp.serialize() {
            info!(target: &self.target_topic, "Serialized response: {data:?}");
            serialized = Self::compress(data, &resp.compression_type).map(fragment_response);
            info!(target: &self.target_topic, "Compressed data: {serialized:?}");
        } else {
            error!(target: &self.target_topic, "Cannot serialize response {resp:?}, dropping response");
            return;
        }

        match serialized {
            Ok(data) => {
                let sz: usize = data.len();
                if let Some(next_hop) = self.packet_send.get(&resp_hdr.hops[1]) {
                    for (i, frag) in data.into_iter().enumerate() {
                        let sid: u64 = (self.session_id << 16) | u64::from(rid);
                        // wtf is this constructor???
                        let packet: Packet = Packet::new_fragment(
                            resp_hdr.clone(),
                            sid,
                            Fragment::new(i as u64, sz as u64, frag),
                        );
                        self.sent_history
                            .insert(sid, (src_id, i as u64, sz as u64, frag));
                        self.session_id = (self.session_id + 1) & SID_MASK;
                        let _ = next_hop.send(packet.clone());
                        let _ = self.controller_send.send(ServerEvent::PacketSent(packet));
                    }
                } else {
                    for (i, frag) in data.into_iter().enumerate() {
                        let sid: u64 = (self.session_id << 16) | u64::from(rid);
                        self.sent_history
                            .insert(sid, (src_id, i as u64, sz as u64, frag));
                        self.session_id = (self.session_id + 1) & SID_MASK;
                        self.pending_packets.push_back(sid);
                    }
                    error!(target: &self.target_topic, "Unable to find channel of designated nbr! pending response...");
                }
            }
            Err(_) => {
                error!(target: &self.target_topic, "CRITICAL: Error during serialization of reponse, dropping response");
            }
        }
    }

    pub(crate) fn resend_packet(
        &mut self,
        sid: u64,
        src_id: NodeId,
        i: u64,
        sz: u64,
        frag: [u8; FRAGMENT_DSIZE],
    ) {
        if let Some(p) = self.get_route(src_id) {
            let packet: Packet = Packet::new_fragment(
                SourceRoutingHeader::new(p, 1),
                sid,
                Fragment::new(i, sz, frag),
            );
            self.packet_send
                .get(&packet.routing_header.hops[1])
                .map_or_else(
                    || {
                        error!(target: &self.target_topic, "CRITICAL: Unable to find channel of designated nbr!, putting in queue!");
                        self.graph_updated = false;
                        self.pending_packets.push_back(sid);
                    },
                    |c| {
                        let _ = c.send(packet.clone());
                        let _ = self.controller_send.send(ServerEvent::PacketSent(packet));
                    },
                );
        } else {
            warn!(target: &self.target_topic, "Failed to resend packet with sid: {sid}");
            self.graph_updated = false;
            self.pending_packets.push_back(sid);
        }
    }
}
