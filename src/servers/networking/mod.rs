use common::{networking::flooder::Flooder, ring_buffer::RingBuffer, slc_commands::ServerEvent};
use crossbeam_channel::Sender;
use log::{error, info, warn};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{FloodRequest, FloodResponse, NodeType, Packet},
};

use super::{GenericServer, SID_MASK};

mod routing;

impl Flooder for GenericServer {
    const NODE_TYPE: NodeType = NodeType::Server;

    #[inline]
    fn get_id(&self) -> NodeId {
        self.id
    }

    #[inline]
    fn get_neighbours(&self) -> impl ExactSizeIterator<Item = (&NodeId, &Sender<Packet>)> {
        self.packet_send.iter()
    }

    fn has_seen_flood(&self, flood_id: (NodeId, u64)) -> bool {
        self.flood_history
            .get(&flood_id.0)
            .map_or(false, |r| r.contains(&flood_id.1))
    }

    fn insert_flood(&mut self, flood_id: (NodeId, u64)) {
        self.flood_history
            .entry(flood_id.0)
            .or_insert(RingBuffer::with_capacity(64))
            .insert(flood_id.1);
    }

    #[inline]
    fn send_to_controller(&self, p: Packet) {
        let _ = self.controller_send.send(ServerEvent::PacketSent(p));
    }
}

impl GenericServer {
    pub(crate) fn handle_flood_response(
        &mut self,
        mut srch: SourceRoutingHeader,
        sid: u64,
        fr: FloodResponse,
    ) {
        match fr.path_trace.first() {
            Some((id, _)) if *id == self.id => {
                self.update_network_from_flood(&fr);
                self.graph_updated = true;
            }
            Some(_) => match srch.next_hop() {
                Some(next_id) => {
                    srch.increase_hop_index();
                    let packet: Packet = Packet::new_flood_response(srch, sid, fr);
                    if let Some(c) = self.packet_send.get(&next_id) {
                        info!("Forwarding flood response");
                        let _ = c.send(packet.clone());
                        let _ = self.controller_send.send(ServerEvent::PacketSent(packet));
                    } else {
                        warn!("Forwarding ill formed (wrong src header) flood response using shortcut");
                        let _ = self.controller_send.send(ServerEvent::ShortCut(packet));
                    }
                }
                None => {
                    error!("Received flood response with invalid header: {srch}");
                }
            },
            None => {
                error!("Found flood response with empty source routing header, ignoring...");
            }
        }
    }

    pub(crate) fn flood(&mut self) {
        let flood = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest::initialize(self.session_id, self.id, NodeType::Server),
        );
        self.session_id = (self.session_id + 1) & SID_MASK;
        for c in self.packet_send.values() {
            let _ = c.send(flood.clone());
        }
        let _ = self.controller_send.send(ServerEvent::PacketSent(flood));
    }
}
