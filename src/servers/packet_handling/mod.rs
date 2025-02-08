use common::slc_commands::ServerEvent;
use log::{error, info, warn};
use std::vec;
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Ack, Fragment, Nack, NackType, Packet, FRAGMENT_DSIZE},
};

use super::{GenericServer, RequestHandler, ServerType};
use crate::{protocol_utils as network_protocol, servers::HistoryEntry};

/// testing module
#[cfg(test)]
mod test;

impl<T: ServerType> GenericServer<T>
where
    GenericServer<T>: RequestHandler,
{
    /// removes the acknowledged [Packet] from the sent history and updates
    /// the pdr of the drones
    pub(super) fn handle_ack(&mut self, sid: u64, _ack: &Ack) {
        if let Some(entry) = self.sent_history.remove(&sid) {
            self.update_pdr_from_ack(&entry.hops);
            info!(target: &self.target_topic, "Sid: {sid} acknoledged");
        } else {
            warn!(target: &self.target_topic, "Received unknow sid in Ack msg: {sid}");
        }
    }

    /// tries to resend the lost [Packet] and, in case of [NackType::ErrorInRouting], if updates
    /// the pdr accordingly
    pub(super) fn handle_nack(&mut self, sid: u64, srch: &SourceRoutingHeader, nack: &Nack) {
        info!(target: &self.target_topic, "Handling received nack: {nack}");
        match nack.nack_type {
            NackType::Dropped => {
                self.update_pdr_from_nack(&srch.hops);
                info!(target: &self.target_topic, "Received dropped nack, updating pdr");
            }
            NackType::ErrorInRouting(id) => {
                self.network_graph.remove_node(id);
                self.need_flood = true;
            }
            NackType::DestinationIsDrone => {
                error!(target: &self.target_topic, "CRITICAL: sent a message with drone as destination?");
            }
            NackType::UnexpectedRecipient(_) => {
                warn!(target: &self.target_topic, "Care, a drone reported unexpected recipient?");
            }
        }

        let fragment: Option<&HistoryEntry> = self.sent_history.get(&sid);

        if let Some(entry) = fragment {
            let HistoryEntry {
                hops: _,
                receiver_id,
                frag_idx,
                n_frags,
                frag,
            } = *entry;
            self.resend_packet(sid, receiver_id, frag_idx, n_frags, frag);
        } else {
            warn!(target: &self.target_topic, "Received Nack with unknown sid: {sid}");
        }
    }

    /// handles a received fragment, if the fragment was the last one needed to reconstruct a request
    /// the request is also handled
    #[allow(clippy::cast_possible_truncation)]
    pub(super) fn handle_fragment(
        &mut self,
        srch: &SourceRoutingHeader,
        sid: u64,
        frag: &Fragment,
    ) {
        let rid: u16 = network_protocol::get_rid(sid);
        if let Some(&id) = srch.hops.first() {
            let entry: &mut (u64, Vec<[u8; FRAGMENT_DSIZE]>) =
                self.fragment_history.entry((id, rid)).or_insert((
                    0,
                    // fine on 64 bit machines
                    vec![[0; FRAGMENT_DSIZE]; frag.total_n_fragments as usize],
                ));
            entry.1.get_mut(frag.fragment_index as usize).map_or_else(
                || warn!(target: &self.target_topic, "Received fragment with invalid index"),
                |v: &mut [u8; FRAGMENT_DSIZE]| {
                    entry.0 += 1;
                    *v = frag.data;
                },
            );
            if entry.0 == frag.total_n_fragments {
                info!(target: &self.target_topic, "All fragments received, reconstructing request {rid}");
                let data: Vec<[u8; FRAGMENT_DSIZE]> =
                    self.fragment_history.remove(&(id, rid)).unwrap().1;
                self.handle_request(srch, id, rid, data);
            }
            self.send_ack(srch, srch.hops[0], sid, frag.fragment_index);
        } else {
            error!(target: &self.target_topic, "Received fragment with invalid source routing header!");
        }
    }

    /// sends an [Ack] after a successful reception of a fragment
    /// since [Ack]s are not droppable, if a route is not found, it is sent
    /// using the controller shortcut
    pub(super) fn send_ack(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
        sid: u64,
        frag_idx: u64,
    ) {
        let mut hdr: SourceRoutingHeader = self.get_routing_hdr_with_hint(srch, src_id);
        hdr.increase_hop_index();
        let ack: Packet = Packet::new_ack(hdr, sid, frag_idx);

        info!(target: &self.target_topic, "Sending ack {ack}, receiving route: {srch}");

        if ack.routing_header.len() < 2 {
            error!(target: &self.target_topic,
                "Error, srch of response ack: {}. Dropping response",
                ack.routing_header
            );
            return;
        }

        if let Some(c) = self.packet_send.get(&ack.routing_header.hops[1]) {
            let _ = c.send(ack.clone());
            let _ = self.controller_send.send(ServerEvent::PacketSent(ack));
        } else {
            warn!(target: &self.target_topic, "Can't find Ack route, shortcutting");
            let _ = self.controller_send.send(ServerEvent::ShortCut(ack));
        }
    }
}
