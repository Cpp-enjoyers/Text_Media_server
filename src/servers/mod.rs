/*
 * TODOS: ETX with packet count + exponentially moving average
 *        Cache shortest paths (this conflicts with ETX tho)
 *        Multithreading?
 */

use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
};

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
#[cfg(test)]
mod test;
#[cfg(test)]
mod test_utils;

// maps (SenderId, rid) -> (#recv_fragments, fragments)
type FragmentHistory = HashMap<(NodeId, u16), (u64, Vec<[u8; FRAGMENT_DSIZE]>)>;
// maps sid -> (ReceiverId, frag_idx, #tot_frags, frag)
type MessageHistory = HashMap<u64, (NodeId, u64, u64, [u8; FRAGMENT_DSIZE])>;
type FloodHistory = HashMap<NodeId, RingBuffer<u64>>;
type NetworkGraph = DiGraphMap<NodeId, f64>;
type PendingQueue = VecDeque<u64>;

const SID_MASK: u64 = 0xFFFF_FFFF_FFFF;
const RID_MASK: u64 = 0xFFFF;
const TEXT_PATH: &str = "./public/";
const MEDIA_PATH: &str = "./media/";

pub trait ServerType {}

pub struct Media {}
pub struct Text {}

impl ServerType for Media {}
impl ServerType for Text {}

pub trait RequestHandler {
    fn handle_request(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
        rid: u16,
        data: Vec<[u8; FRAGMENT_DSIZE]>,
    );
}

pub type TextServer = GenericServer<Text>;
pub type MediaServer = GenericServer<Media>;

pub struct GenericServer<T: ServerType> {
    id: NodeId,
    target_topic: String,
    session_id: u64, // wraps around 48 bits
    need_flood: bool,
    graph_updated: bool,
    controller_send: Sender<ServerEvent>,
    controller_recv: Receiver<ServerCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    flood_history: FloodHistory,
    fragment_history: FragmentHistory,
    sent_history: MessageHistory,
    network_graph: NetworkGraph,
    pending_packets: PendingQueue,
    _marker: PhantomData<T>,
}

impl<T: ServerType> GenericServer<T>
where
    GenericServer<T>: RequestHandler,
{
    fn handle_packet(&mut self, packet: Packet) {
        let srch: SourceRoutingHeader = packet.routing_header;
        let sid: u64 = packet.session_id;
        match packet.pack_type {
            PacketType::MsgFragment(frag) => {
                info!(target: &self.target_topic, "Received message fragment {frag}");
                self.handle_fragment(&srch, sid, &frag);
            }
            PacketType::Ack(ack) => {
                info!(target: &self.target_topic, "Received ack {ack}");
                self.handle_ack(sid, &ack);
            }
            PacketType::Nack(nack) => {
                info!(target: &self.target_topic, "Received nack {nack}");
                self.handle_nack(sid, &nack);
            }
            PacketType::FloodRequest(mut fr) => {
                info!(target: &self.target_topic, "Received flood request {fr}");
                if let Ok(()) = self.handle_flood_request(&srch, sid, &mut fr) {
                    info!(target: &self.target_topic, "Flood request handled properly");
                } else {
                    warn!(target: &self.target_topic, "Error during flood request handling, dropping packet");
                }
            }
            PacketType::FloodResponse(fr) => {
                info!(target: &self.target_topic, "Received flood response {fr}");
                self.handle_flood_response(srch, sid, fr);
            }
        }
    }

    fn handle_command(&mut self, command: ServerCommand) {
        match command {
            ServerCommand::AddSender(node_id, channel) => {
                self.packet_send.insert(node_id, channel);
                self.network_graph.add_edge(self.id, node_id, 1.);
                self.network_graph.add_edge(node_id, self.id, 1.);
                self.need_flood = true;
                info!(target: &self.target_topic, "Received add sender command, sender id: {node_id}");
            }
            ServerCommand::RemoveSender(node_id) => {
                self.packet_send.remove(&node_id);
                self.network_graph.remove_node(node_id);
                self.need_flood = true;
                info!(target: &self.target_topic, "Received remove sender command, sender id: {node_id}");
            }
            ServerCommand::Shortcut(p) => {
                info!(target: &self.target_topic, "Received packet {p} from controller shortcut");
                self.handle_packet(p);
            }
        }
    }
}

impl<T: ServerType> Server for GenericServer<T>
where
    GenericServer<T>: RequestHandler,
{
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
            network_graph.add_edge(id, *did, 1.);
            // network_graph.add_edge(*did, id, 1.);
        }

        GenericServer {
            id,
            target_topic: format!("Server[{id}]"),
            session_id: 0,
            need_flood: true,
            graph_updated: false,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            flood_history: HashMap::new(),
            fragment_history: HashMap::new(),
            sent_history: HashMap::new(),
            network_graph,
            pending_packets: VecDeque::new(),
            _marker: PhantomData,
        }
    }

    fn run(&mut self) {
        loop {
            if self.need_flood {
                info!(target: &self.target_topic, "Starting new flood request to construct network");
                self.flood();
            } else if self.graph_updated && !self.pending_packets.is_empty() {
                if let Some(sid) = self.pending_packets.pop_back() {
                    info!(target: &self.target_topic, "Trying to resend packet with sid: {sid}");
                    let fragment: Option<&(u8, u64, u64, [u8; FRAGMENT_DSIZE])> =
                        self.sent_history.get(&sid);

                    if let Some(t) = fragment {
                        let (src_id, i, sz, frag) = *t;
                        self.resend_packet(sid, src_id, i, sz, frag);
                    }
                }
            } else {
                select_biased! {
                    recv(self.controller_recv) -> command => {
                        if let Ok(command) = command {
                            self.handle_command(command);
                        }
                    },
                    recv(self.packet_recv) -> packet => {
                        if let Ok(packet) = packet {
                            self.handle_packet(packet);
                        }
                    }
                }
            }
        }
    }
}
