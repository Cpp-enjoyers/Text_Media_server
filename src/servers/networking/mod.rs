use common::{networking::flooder::Flooder, ring_buffer::RingBuffer, slc_commands::ServerEvent};
use wg_2024::packet::NodeType;

use super::GenericServer;

mod routing;

impl Flooder for GenericServer {
    const NODE_TYPE: wg_2024::packet::NodeType = NodeType::Server;

    #[inline]
    fn get_id(&self) -> wg_2024::network::NodeId {
        self.id
    }

    #[inline]
    fn get_neighbours(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &wg_2024::network::NodeId,
            &crossbeam_channel::Sender<wg_2024::packet::Packet>,
        ),
    > {
        self.packet_send.iter()
    }

    fn has_seen_flood(&self, flood_id: (wg_2024::network::NodeId, u64)) -> bool {
        self.flood_history
            .get(&flood_id.0)
            .map_or(false, |r| r.contains(&flood_id.1))
    }

    fn insert_flood(&mut self, flood_id: (wg_2024::network::NodeId, u64)) {
        self.flood_history
            .entry(flood_id.0)
            .or_insert(RingBuffer::with_capacity(64))
            .insert(flood_id.1);
    }

    #[inline]
    fn send_to_controller(&self, p: wg_2024::packet::Packet) {
        let _ = self.controller_send.send(ServerEvent::PacketSent(p));
    }
}
