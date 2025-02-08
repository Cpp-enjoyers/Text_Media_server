use std::{
    fs::{self, read},
    io,
    path::PathBuf,
};

use common::{
    slc_commands::{ServerEvent, ServerType},
    web_messages::{
        Compression, MediaRequest, Request, ResponseMessage, Serializable, SerializableSerde,
        TextRequest,
    },
};
use compression::{
    bypass::BypassCompressor, huffman::HuffmanCompressor, lzw::LZWCompressor, Compressor,
};
use log::{error, info, warn};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Fragment, Packet, FRAGMENT_DSIZE},
};

use super::{
    serialization::{defragment_deserialize_request, fragment_response},
    GenericServer, HistoryEntry, Media, RequestHandler, Text,
};

use crate::protocol_utils as network_protocol;
use crate::servers::{ServerType as ST, MEDIA_PATH, TEXT_PATH};

/// testing module
#[cfg(test)]
mod test;

/// lists the contents of a directory
fn list_dir(path: &str) -> Result<Vec<String>, io::Error> {
    Ok(fs::read_dir(path)?
        .filter(Result::is_ok)
        .map(|p: Result<fs::DirEntry, io::Error>| p.unwrap().path())
        .filter(|p: &PathBuf| p.is_file())
        .map(|p: PathBuf| p.into_os_string().into_string().unwrap())
        .collect())
}

impl<T: ST> GenericServer<T> {
    /// compresses the data based on the requested type
    fn compress(data: Vec<u8>, comp: &Compression) -> Result<Vec<u8>, String> {
        match comp {
            Compression::Huffman => HuffmanCompressor::new()
                .compress(data)?
                .serialize()
                .map_err(|_| "Error during compression".to_string()),
            Compression::LZW => Serializable::serialize(&LZWCompressor::new().compress(data)?)
                .map_err(|_| "Error during compression".to_string()),
            Compression::None => BypassCompressor::new().compress(data),
        }
    }

    /// send response realted to a fully received request.
    /// the response will have the same rid of the response as required by the protocol
    pub(super) fn send_response(
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
            info!(target: &self.target_topic, "Serialized response");
            serialized = Self::compress(data, &resp.compression_type).map(fragment_response);
            info!(target: &self.target_topic, "Compressed data");
        } else {
            error!(target: &self.target_topic, "Cannot serialize response {resp:?}, dropping response");
            return;
        }

        match serialized {
            Ok(data) => {
                let sz: usize = data.len();
                if let Some(next_hop) = self.packet_send.get(&resp_hdr.hops[1]) {
                    for (i, frag) in data.into_iter().enumerate() {
                        let sid: u64 = network_protocol::generate_response_id(self.session_id, rid);
                        let packet: Packet = Packet::new_fragment(
                            resp_hdr.clone(),
                            sid,
                            Fragment::new(i as u64, sz as u64, frag),
                        );
                        // (src_id, i as u64, sz as u64, frag)
                        self.sent_history.insert(
                            sid,
                            HistoryEntry::new(
                                resp_hdr.hops.clone(),
                                src_id,
                                i as u64,
                                sz as u64,
                                frag,
                            ),
                        );
                        self.session_id = network_protocol::next_sid(self.session_id);
                        let _ = next_hop.send(packet.clone());
                        let _ = self.controller_send.send(ServerEvent::PacketSent(packet));
                    }
                } else {
                    // no route, send to pending queue
                    for (i, frag) in data.into_iter().enumerate() {
                        let sid: u64 = (self.session_id << 16) | u64::from(rid);
                        self.sent_history.insert(
                            sid,
                            HistoryEntry::new(
                                resp_hdr.hops.clone(),
                                src_id,
                                i as u64,
                                sz as u64,
                                frag,
                            ),
                        );
                        self.session_id = network_protocol::next_sid(self.session_id);
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

    /// tries to re send a packet in the pending queue, if it fails this won't be tried again untile the next
    /// flood
    pub(super) fn resend_packet(
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
                        self.sent_history.entry(sid).and_modify(|e: &mut HistoryEntry| e.hops.clone_from(&packet.routing_header.hops));
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

/// [super::TextServer] specialization code
impl RequestHandler for GenericServer<Text> {
    fn handle_request(
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
                            list_dir(TEXT_PATH).unwrap_or_default(),
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
            info!(target: &self.target_topic, "Sending response");
            self.send_response(srch, src_id, rid, &resp);
        } else {
            error!(target: &self.target_topic, "Received undeserializable request, dropping request...");
        }
        // self.session_id = (self.session_id + 1) & SID_MASK;
    }
}

/// [super::MediaServer] specialization code
impl RequestHandler for GenericServer<Media> {
    fn handle_request(
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
                        ServerType::MediaServer,
                    );
                }
                Request::Media(mr) => match mr {
                    MediaRequest::MediaList => {
                        resp = ResponseMessage::new_media_list_response(
                            self.id,
                            req.compression_type,
                            list_dir(MEDIA_PATH).unwrap_or_default(),
                        );
                    }
                    MediaRequest::Media(str) => {
                        resp = if let Ok(data) = read(str) {
                            ResponseMessage::new_media_response(self.id, req.compression_type, data)
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
            info!(target: &self.target_topic, "Sending response");
            self.send_response(srch, src_id, rid, &resp);
        } else {
            error!(target: &self.target_topic, "Received undeserializable request, dropping request...");
        }
    }
}
