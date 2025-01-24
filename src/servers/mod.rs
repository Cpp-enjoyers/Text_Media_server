use std::collections::HashMap;

use common::{
    networking::flooder::Flooder, ring_buffer::RingBuffer, slc_commands::{ServerCommand, ServerEvent}, Server
};
use crossbeam_channel::{select_biased, Receiver, Sender};
use log::{info, warn};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Packet, PacketType, FRAGMENT_DSIZE},
};

mod networking;
mod packet_handling;

pub struct GenericServer {
    id: NodeId,
    session_id: u64,
    controller_send: Sender<ServerEvent>,
    controller_recv: Receiver<ServerCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    flood_history: HashMap<NodeId, RingBuffer<u64>>,
    // ! Change types to fit this random crap
    fragment_history: HashMap<(NodeId, u64), Vec<[u8; FRAGMENT_DSIZE]>>,
    sent_history: HashMap<u64, (usize, Vec<[u8; FRAGMENT_DSIZE]>)>,
}

impl GenericServer {
    fn handle_packet(&mut self, packet: Packet) {
        let srch: &SourceRoutingHeader = &packet.routing_header;
        let sid = packet.session_id;
        match packet.pack_type {
            PacketType::MsgFragment(frag) => todo!(),
            PacketType::Ack(ack) => {
                self.handle_ack(sid, &ack);
            },
            PacketType::Nack(nack) => todo!(),
            PacketType::FloodRequest(mut fr) => {
                match self.handle_flood_request(srch, sid, &mut fr) {
                    Ok(_) => info!("Flood request handled properly"),
                    Err(_) => warn!("Error during flood request handling, dropping packet"),
                }
            },
            PacketType::FloodResponse(fr) => todo!(),
        }
    }

    fn handle_command(&mut self, command: ServerCommand) {
        match command {
            ServerCommand::AddSender(node_id, channel) => {
                self.packet_send.insert(node_id, channel);
            }
            ServerCommand::RemoveSender(node_id) => {
                self.packet_send.remove(&node_id);
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
