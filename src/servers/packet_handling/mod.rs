use common::slc_commands::ServerEvent;
use log::{error, info, warn};
use std::vec;
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Ack, Fragment, Nack, NackType, Packet, FRAGMENT_DSIZE},
};

use super::{GenericServer, RID_MASK};

#[cfg(test)]
mod test;

impl GenericServer {
    #[inline]
    pub(super) fn get_rid(sid: u64) -> u16 {
        // intentional, if shifted by 48 it fits into 16
        u16::try_from(sid & RID_MASK).unwrap()
    }

    pub(crate) fn handle_ack(&mut self, sid: u64, _ack: &Ack) {
        self.sent_history.remove(&sid).map_or_else(
            || warn!(target: &self.target_topic, "Received unknow sid in Ack msg: {sid}"),
            |_| info!("Sid: {sid} acknoledged"),
        );
    }

    pub(crate) fn handle_nack(&mut self, sid: u64, nack: &Nack) {
        info!("Handling received nack: {nack}");
        match nack.nack_type {
            NackType::Dropped => {
                // TODO
                info!(target: "TODO", "ETX?");
            }
            NackType::ErrorInRouting(id) => {
                self.network_graph.remove_node(id);
            }
            NackType::DestinationIsDrone => {
                error!(target: &self.target_topic, "CRITICAL: sent a message with drone as destination?");
            }
            NackType::UnexpectedRecipient(_) => {}
        }

        let fragment: Option<&(u8, u64, u64, [u8; FRAGMENT_DSIZE])> = self.sent_history.get(&sid);

        if let Some(t) = fragment {
            let (src_id, i, sz, frag) = *t;
            self.resend_packet(sid, src_id, i, sz, frag);
        } else {
            warn!(target: &self.target_topic, "Received Nack with unknown sid: {sid}");
        }

        self.need_flood = true;
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn handle_fragment(
        &mut self,
        srch: &SourceRoutingHeader,
        sid: u64,
        frag: &Fragment,
    ) {
        let rid: u16 = Self::get_rid(sid);
        if let Some(&id) = srch.hops.first() {
            let entry: &mut (u64, Vec<[u8; FRAGMENT_DSIZE]>) =
                self.fragment_history.entry((id, rid)).or_insert((
                    0,
                    // should be fine on 64 bit machines
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
                info!("All fragments received, reconstructing request {rid}");
                let data: Vec<[u8; FRAGMENT_DSIZE]> =
                    self.fragment_history.remove(&(id, rid)).unwrap().1;
                self.handle_request(srch, id, rid, data);
            }
            self.send_ack(srch, srch.hops[0], sid, frag.fragment_index);
        } else {
            error!(target: &self.target_topic, "Received fragment with invalid source routing header!");
        }
    }

    pub(crate) fn send_ack(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
        sid: u64,
        frag_idx: u64,
    ) {
        let mut hdr: SourceRoutingHeader = self.get_routing_hdr_with_hint(srch, src_id);
        hdr.increase_hop_index();
        let ack: Packet = Packet::new_ack(hdr, sid, frag_idx);

        info!("Sending ack {ack}, receiving route: {srch}");

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
