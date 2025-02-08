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

/// Module containing the necessary netowrking functions to discover the network
mod networking;
/// Module containing the necessary functions to handle received packets
mod packet_handling;
/// Module containing the necessary functions to handle received requests and
/// handle/create associated responses
mod requests_handling;
/// Module containing the necessary routing functions to find route paths and
/// estimate drone ETXs
mod routing;
/// Module containing auxiliary functions for the serialization and deserialization
/// of received/sended packets
mod serialization;
/// Test module
#[cfg(test)]
mod test;
/// Common utilities for testing
#[cfg(test)]
mod test_utils;

/// Struct containing the necessary information to update and resend a packet in case of a Nack
#[derive(Debug, Clone)]
struct HistoryEntry {
    /// Routing header used to send the packet
    /// This is useful to update the ETX of the drones accordingly
    hops: Vec<NodeId>,
    /// Node id of the receiver
    receiver_id: NodeId,
    /// Index of the fragment in the response
    frag_idx: u64,
    /// Total number of fragments in the response
    n_frags: u64,
    /// The actual fragment
    frag: [u8; 128],
}

impl HistoryEntry {
    /// Creates a new [HistoryEntry] from the given parameters
    #[inline]
    #[must_use]
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

/// Data structure used to handle received fragments and map them to the related
/// request id
/// maps (SenderId, rid) -> (#recv_fragments, fragments)
type FragmentHistory = HashMap<(NodeId, u16), (u64, Vec<[u8; FRAGMENT_DSIZE]>)>;
/// Data structure used to cache sended packets that are yet to be acknowledged
type MessageHistory = HashMap<u64, HistoryEntry>;
/// Data structure used to remember already seen flood ids
type FloodHistory = HashMap<NodeId, RingBuffer<u64>>;
/// Used graph to represent the network
/// the graph is directional with NodeIds as nodes and f64 as waights
type NetworkGraph = DiGraphMap<NodeId, f64>;
/// Queue of packets pending: these are the packets waiting to be sent due
/// to the server not being able to find a route when they were handled
type PendingQueue = VecDeque<u64>;

/// path of the [TextServer] files
const TEXT_PATH: &str = "./public/";
/// path of the [MediaServer] files
const MEDIA_PATH: &str = "./media/";
/// Initial pdr assigned to the drone (note PDR = 1 / ETX), we use a uniform approach so
/// the initial value is 0.5. another approach could be to set the initial value to
/// Beta(1, 1) and follow the baesyan approach
const INITIAL_PDR: f64 = 0.5; // Beta(1, 1), is a baesyan approach better?
/// Intial ETX of the drone, depends in [INITIAL_PDR]
const INITIAL_ETX: f64 = 1. / INITIAL_PDR;
/// default window size used by the ETX estimator for the EWMA
const DEFAULT_WINDOW_SZ: u32 = 12;
/// alpha constant of the EWMA
const DEFAULT_ALPHA: f64 = 0.35;
/// beta constant of the EWMA
const DEFAULT_BETA: f64 = 1. - DEFAULT_ALPHA;

/// Marker trait used to represent the `ServerType` of a [GenericServer]
pub trait ServerType {}

/// One of the two default types of a [GenericServer], the [MediaServer]
/// handles requests related to the images contained in the files sent
/// by the [TextServer]
pub struct Media {}
/// One of the two default types of a [GenericServer], the [TextServer]
/// handles file requests. The default format used is html so that also
/// images can be embedded in the document, if needed
pub struct Text {}

impl ServerType for Media {}
impl ServerType for Text {}

/// Trait utilized to speicalise [GenericServer<T: ServerType>]. This trait
/// allows to specify how the server should handle the received protocol
/// requests based on its [ServerType]
pub trait RequestHandler {
    /// Function to implement the desired behaviour of a specialised [GenericServer]
    fn handle_request(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
        rid: u16,
        data: Vec<[u8; FRAGMENT_DSIZE]>,
    );
}

/// Handy type alias for a [`GenericServer<Text>`]
pub type TextServer = GenericServer<Text>;
/// Handy type alias for a [`GenericServer<Media>`]
pub type MediaServer = GenericServer<Media>;

/// Struct containing all the necessary information for a server to correctly
/// handle received packets according to the network protocol. <br>
/// Requires a generic type that implements [ServerType] and the trait [RequestHandler]
/// to implement the desired behaviour in the high level protocol
pub struct GenericServer<T: ServerType> {
    /// id of the node
    id: NodeId,
    /// target topic of the server, used in logs
    target_topic: String,
    /// next session id
    session_id: u64, // wraps around 48 bits
    /// flag to indicate wheter or not the server needs
    /// to start a new flood
    need_flood: bool,
    /// flag to signal an update in the network graph.
    /// this is useful as it allows to know when to try
    /// sending again the pending packets
    graph_updated: bool,
    /// channel to communicate [ServerEvent]s to the controller
    controller_send: Sender<ServerEvent>,
    /// channel to receive [ServerCommand]s from the controller
    controller_recv: Receiver<ServerCommand>,
    /// channel to receive [Packet]s from the drones
    packet_recv: Receiver<Packet>,
    /// map containing the channel to send [Packet]s to the drones
    /// mapped to their relative [NodeId]s
    packet_send: HashMap<NodeId, Sender<Packet>>,
    /// history of the last 64 flood ids seen for each known
    /// initiator
    flood_history: FloodHistory,
    /// history of received fragments, mapped to their rid
    fragment_history: FragmentHistory,
    /// history of sended messages still waiting to be acknowledged
    sent_history: MessageHistory,
    /// the network graph with the necessary estimators for [Packet]
    /// routing
    network_graph: RoutingTable,
    /// queue of [Packet]s waiting to be re sent
    pending_packets: PendingQueue,
    /// marker used to specify the [GenericServer]'s type
    _marker: PhantomData<T>,
}

/// Default estiamtor used by the [GenericServer]: the estimator uses an exponentially
/// weighted moving average (EWMA).
/// the formula is as follows:
///     ETX(n) = p(n) * alpha + ETX(n - 1) * beta
///     ETX(0) = [INITIAL_ETX]
/// where ETX(n) is the ETX at time n
/// p(n) is the estimated ETX at time n, estimated from the last [DEFAULT_WINDOW_SZ] samples
/// alpha and beta are parameters that decide how fast the ETX adapts to change
fn default_estimator() -> PdrEstimator {
    PdrEstimator::new(DEFAULT_WINDOW_SZ, |old: f64, acks: u32, nacks: u32| {
        DEFAULT_ALPHA * (f64::from(acks) / f64::from(acks + nacks)) + DEFAULT_BETA * old
    })
}

impl<T: ServerType> GenericServer<T>
where
    GenericServer<T>: RequestHandler,
{
    /// function to handle a packet based on it's internal type
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

    /// function to handle command based on it's internal type
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
    /// creates a new [GenericServer] from the given network channels
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

    /// main loop of the [GenericServer]
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
