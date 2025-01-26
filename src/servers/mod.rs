use std::collections::HashMap;

use common::{
    networking::flooder::Flooder,
    ring_buffer::RingBuffer,
    slc_commands::{ServerCommand, ServerEvent},
    Server,
};
use crossbeam_channel::{select_biased, Receiver, Sender};
use log::{info, warn};
use petgraph::prelude::DiGraphMap;
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType, FRAGMENT_DSIZE},
};

mod networking;
mod packet_handling;
mod requests_handling;
mod serialization;

type FragmentHistory = HashMap<(NodeId, u16), (u64, Vec<[u8; FRAGMENT_DSIZE]>)>;
type MessageHistory = HashMap<u64, [u8; FRAGMENT_DSIZE]>;
type FloodHistory = HashMap<NodeId, RingBuffer<u64>>;
type NetworkGraph = DiGraphMap<NodeId, f64>;

const SID_MASK: u64 = 0xFFFF_FFFF_FFFF;
const RID_MASK: u64 = 0xFFFF;

pub struct GenericServer {
    id: NodeId,
    session_id: u64, // wraps around 48 bits
    controller_send: Sender<ServerEvent>,
    controller_recv: Receiver<ServerCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    flood_history: FloodHistory,
    fragment_history: FragmentHistory,
    sent_history: MessageHistory,
    network_graph: NetworkGraph,
}

impl GenericServer {
    fn handle_packet(&mut self, packet: Packet) {
        let srch: SourceRoutingHeader = packet.routing_header;
        let sid: u64 = packet.session_id;
        match packet.pack_type {
            PacketType::MsgFragment(frag) => {
                self.handle_fragment(&srch, sid, &frag);
            }
            PacketType::Ack(ack) => {
                self.handle_ack(sid, &ack);
            }
            PacketType::Nack(nack) => {
                self.handle_nack(sid, &nack);
            }
            PacketType::FloodRequest(mut fr) => {
                if let Ok(()) = self.handle_flood_request(&srch, sid, &mut fr) {
                    info!("Flood request handled properly");
                } else {
                    warn!("Error during flood request handling, dropping packet");
                }
            }
            PacketType::FloodResponse(fr) => {
                self.handle_flood_response(srch, sid, fr);
            }
        }
    }

    fn handle_command(&mut self, command: ServerCommand) {
        match command {
            ServerCommand::AddSender(node_id, channel) => {
                self.packet_send.insert(node_id, channel);
                self.network_graph.add_edge(node_id, self.id, 1.);
            }
            ServerCommand::RemoveSender(node_id) => {
                self.packet_send.remove(&node_id);
                self.network_graph.remove_edge(node_id, self.id);
            }
            ServerCommand::Shortcut(p) => self.handle_packet(p),
        }
    }
}

impl Server for GenericServer {
    fn new(
        id: NodeId,
        controller_send: Sender<ServerEvent>,
        controller_recv: Receiver<ServerCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self
    where
        Self: Sized,
    {
        let mut network_graph: DiGraphMap<NodeId, f64> = DiGraphMap::new();
        for did in packet_send.keys() {
            network_graph.add_edge(*did, id, 1.);
        }

        GenericServer {
            id,
            session_id: 0,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            flood_history: HashMap::new(),
            fragment_history: HashMap::new(),
            sent_history: HashMap::new(),
            network_graph,
        }
    }

    fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.controller_recv) -> command => {
                    if let Ok(_command) = command {
                        println!("comando");
                    }
                },
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        println!("{packet:?}");
                    }
                }
            }
        }
    }
}
