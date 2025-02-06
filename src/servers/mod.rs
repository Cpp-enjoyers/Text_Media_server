/*
 * TODOS: ETX with packet count + exponentially moving average
 *        Multithreading (prob there's no time >.<)
 *        Test SimController behaviour
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
use routing::{PdrEstimator, RoutingTable};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType, FRAGMENT_DSIZE},
};

mod networking;
mod packet_handling;
mod requests_handling;
mod routing;
mod serialization;
#[cfg(test)]
mod test;
#[cfg(test)]
mod test_utils;

#[derive(Debug, Clone)]
struct HistoryEntry {
    hops: Vec<NodeId>,
    receiver_id: u8,
    frag_idx: u64,
    n_frags: u64,
    frag: [u8; 128],
}

impl HistoryEntry {
    fn new(
        hops: Vec<NodeId>,
        receiver_id: u8,
        frag_idx: u64,
        n_frags: u64,
        frag: [u8; 128],
    ) -> Self {
        Self {
            hops,
            receiver_id,
            frag_idx,
            n_frags,
            frag,
        }
    }
}

// maps (SenderId, rid) -> (#recv_fragments, fragments)
type FragmentHistory = HashMap<(NodeId, u16), (u64, Vec<[u8; FRAGMENT_DSIZE]>)>;
type MessageHistory = HashMap<u64, HistoryEntry>;
type FloodHistory = HashMap<NodeId, RingBuffer<u64>>;
type NetworkGraph = DiGraphMap<NodeId, f64>;
type PendingQueue = VecDeque<u64>;

const TEXT_PATH: &str = "./public/";
const MEDIA_PATH: &str = "./media/";
const INITIAL_PDR: f64 = 0.5; // Beta(1, 1), is a baesyan approach better?
const INITIAL_ETX: f64 = 1. / INITIAL_PDR;
const DEFAULT_WINDOW_SZ: u32 = 12;
const DEFAULT_ALPHA: f64 = 0.35;
const DEFAULT_BETA: f64 = 1. - DEFAULT_ALPHA;

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
    network_graph: RoutingTable,
    pending_packets: PendingQueue,
    _marker: PhantomData<T>,
}

fn default_estimator() -> PdrEstimator {
    PdrEstimator::new(DEFAULT_WINDOW_SZ, |old: f64, acks: u32, nacks: u32| {
        DEFAULT_ALPHA * (f64::from(acks) / f64::from(acks + nacks)) + DEFAULT_BETA * old
    })
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
                self.handle_nack(sid, &srch, &nack);
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
                self.check_and_add_edge(self.id, node_id);
                // self.network_graph.check_and_add_edge(node_id, self.id);
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
        let mut network_graph: RoutingTable = RoutingTable::new(default_estimator());
        for did in packet_send.keys() {
            network_graph.check_and_add_edge(id, *did);
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
                let sid = self.pending_packets.pop_back().unwrap();
                info!(target: &self.target_topic, "Trying to resend packet with sid: {sid}");
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
                    warn!(target: &self.target_topic, "CRITICAL: cannot find pending packet in sent history!");
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
