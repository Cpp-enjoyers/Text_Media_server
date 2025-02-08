#[cfg(test)]
mod networking_tests {
    use std::{collections::HashMap, thread, time::Duration};

    use ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone;
    use common::{
        networking::flooder::Flooder,
        slc_commands::{ServerCommand, ServerEvent},
        Server,
    };
    use crossbeam_channel::{unbounded, RecvError, Sender};
    use wg_2024::{
        drone::Drone,
        network::{NodeId, SourceRoutingHeader},
        packet::{FloodResponse, Nack, NackType, NodeType, Packet, PacketType},
    };

    use crate::servers::{
        test_utils::graphmap_eq, GenericServer, HistoryEntry, NetworkGraph, Text, INITIAL_ETX,
    };

    use crate::servers::test_utils::get_dummy_server_text;

    /// tests correct behaviour of the flood buffer
    #[test]
    fn test_flood_buffer() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        assert!(!server.has_seen_flood((1, 64)));
        server.insert_flood((0, 0));
        assert!(server.has_seen_flood((0, 0)));
        assert!(server.flood_history.contains_key(&0));
        assert!(server.flood_history.get(&0).unwrap().contains(&0));
        server.insert_flood((0, 1));
        assert!(server.flood_history.get(&0).unwrap().contains(&0));
        assert!(server.flood_history.get(&0).unwrap().contains(&1));
    }

    /// tests correct use of the controller shortcuts
    #[test]
    fn test_send_to_controller() {
        let (ctrl_send, ctrl_recv_ev) = crossbeam_channel::unbounded();
        let (_, ctrl_recv) = crossbeam_channel::unbounded();
        let (_, server_recv) = crossbeam_channel::unbounded();
        let server: GenericServer<Text> =
            GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
        let dummy_pkt: Packet = Packet::new_ack(SourceRoutingHeader::empty_route(), 0, 0);
        server.send_to_controller(dummy_pkt.clone());
        assert!(ctrl_recv_ev.recv().unwrap() == ServerEvent::PacketSent(dummy_pkt));
        assert!(ctrl_recv_ev.recv_timeout(Duration::from_secs(1)).is_err());
    }

    /// tests correct handling of [FloodResponse]s initiated by the server
    #[test]
    fn test_handle_my_flood_response() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let response: FloodResponse = FloodResponse {
            flood_id: 0,
            path_trace: vec![
                (0, NodeType::Server),
                (1, NodeType::Drone),
                (2, NodeType::Client),
            ],
        };
        server.handle_flood_response(SourceRoutingHeader::new(vec![2, 1, 0], 2), 0, response);
        assert!(graphmap_eq(
            &server.network_graph.get_graph(),
            &NetworkGraph::from_edges([(0, 1, INITIAL_ETX), (1, 2, INITIAL_ETX),])
        ));
    }

    /// tests correct handling of [FloodResponse]s not initiated by the server
    #[test]
    fn test_handle_flood_response() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let response: FloodResponse = FloodResponse {
            flood_id: 0,
            path_trace: vec![
                (2, NodeType::Client),
                (1, NodeType::Drone),
                (0, NodeType::Server),
                (3, NodeType::Drone),
            ],
        };
        let (ds, dr) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        server.handle_flood_response(SourceRoutingHeader::new(vec![3, 0, 1, 2], 1), 0, response);
        let data: Result<Packet, RecvError> = dr.recv();
        assert!(data.is_ok());
        let packet: Packet = data.unwrap();
        assert!(packet.routing_header.hop_index == 2);
    }

    /// tests correct [FloodResponse]s shortcutting in the event of ill formed traces
    #[test]
    fn test_handle_flood_response_to_scl() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let response: FloodResponse = FloodResponse {
            flood_id: 0,
            path_trace: vec![
                (2, NodeType::Client),
                (1, NodeType::Drone),
                (0, NodeType::Server),
                (3, NodeType::Drone),
            ],
        };
        let (scls, sclr) = crossbeam_channel::unbounded();
        server.controller_send = scls.clone();
        server.handle_flood_response(SourceRoutingHeader::new(vec![3, 0, 1, 2], 1), 0, response);
        let data: Result<ServerEvent, RecvError> = sclr.recv();
        assert!(data.is_ok());
        let packet: ServerEvent = data.unwrap();
        match packet {
            ServerEvent::ShortCut(p) => assert!(p.routing_header.hop_index == 2),
            _ => panic!(),
        }
    }

    /// test flood network construction in a small topology
    /// 1
    ///  \
    ///   11--12
    ///  /  \
    /// 14--13
    /// 1: Server, [11, 12, 13, 14]: Drones
    #[test]
    fn test_flood_small_topology() {
        // Server channels
        let (c_send, c_recv) = unbounded();
        // Drone 11 channels
        let (d_send, d_recv) = unbounded();
        // Drone 12 channels
        let (d12_send, d12_recv) = unbounded();
        // Drone 13 channels
        let (d13_send, d13_recv) = unbounded();
        // Drone 14 channels
        let (d14_send, d14_recv) = unbounded();
        // SC channels - needed to not make the drone crash
        let (_d_command_send, d_command_recv) = unbounded();
        let (d_event_send, _d_event_rec) = unbounded();
        let (s_event_send, _) = unbounded();
        let (_, s_command_recv) = unbounded();

        // Drone 11
        let neighbours11: HashMap<NodeId, Sender<Packet>> = HashMap::from([
            (12, d12_send.clone()),
            (13, d13_send.clone()),
            (14, d14_send.clone()),
            (1, c_send.clone()),
        ]);
        let mut drone11: CppEnjoyersDrone = CppEnjoyersDrone::new(
            11,
            d_event_send.clone(),
            d_command_recv.clone(),
            d_recv.clone(),
            neighbours11,
            0.0,
        );
        // Drone 12
        let neighbours12: HashMap<NodeId, Sender<Packet>> = HashMap::from([(11, d_send.clone())]);
        let mut drone12: CppEnjoyersDrone = CppEnjoyersDrone::new(
            12,
            d_event_send.clone(),
            d_command_recv.clone(),
            d12_recv.clone(),
            neighbours12,
            0.0,
        );
        // Drone 13
        let neighbours13: HashMap<NodeId, Sender<Packet>> =
            HashMap::from([(11, d_send.clone()), (14, d14_send.clone())]);
        let mut drone13: CppEnjoyersDrone = CppEnjoyersDrone::new(
            13,
            d_event_send.clone(),
            d_command_recv.clone(),
            d13_recv.clone(),
            neighbours13,
            0.0,
        );
        // Drone 14
        let neighbours14: HashMap<NodeId, Sender<Packet>> =
            HashMap::from([(11, d_send.clone()), (13, d13_send.clone())]);
        let mut drone14: CppEnjoyersDrone = CppEnjoyersDrone::new(
            14,
            d_event_send.clone(),
            d_command_recv.clone(),
            d14_recv.clone(),
            neighbours14,
            0.0,
        );

        // server
        let neighbors_s: HashMap<u8, Sender<Packet>> = HashMap::from([(11, d_send.clone())]);
        let mut server: GenericServer<Text> = GenericServer::new(
            1,
            s_event_send.clone(),
            s_command_recv,
            c_recv.clone(),
            neighbors_s,
        );

        // Spawn the drone's run method in a separate thread
        thread::spawn(move || {
            drone11.run();
        });

        thread::spawn(move || {
            drone12.run();
        });

        thread::spawn(move || {
            drone13.run();
        });

        thread::spawn(move || {
            drone14.run();
        });

        server.flood();
        while let Ok(p) = server.packet_recv.recv_timeout(Duration::from_secs(1)) {
            match p.pack_type {
                PacketType::FloodResponse(_) | PacketType::FloodRequest(_) => {
                    server.handle_packet(p);
                }
                _ => panic!(),
            }
        }

        assert!(graphmap_eq(
            &server.network_graph.get_graph(),
            &NetworkGraph::from_edges([
                (1, 11, INITIAL_ETX),
                (11, 12, INITIAL_ETX),
                (12, 11, INITIAL_ETX),
                (11, 13, INITIAL_ETX),
                (13, 11, INITIAL_ETX),
                (13, 14, INITIAL_ETX),
                (14, 13, INITIAL_ETX),
                (11, 14, INITIAL_ETX),
                (14, 11, INITIAL_ETX),
            ])
        ));
    }

    /// test flood network construction in a big topology
    /// 1--12
    /// \  /
    ///  11--13
    ///   \ /  \
    ///    14-- 2
    /// [1, 2]: Server, [11, 12, 13, 14]: Drones
    #[test]
    fn test_flood_big_topology() {
        // env::set_var("RUST_LOG", "info");
        // let _ = env_logger::try_init();

        // Server 1 channels
        let (s_send, s_recv) = unbounded();
        // Server 2 channels
        let (s2_send, s2_recv) = unbounded();
        // Drone 11
        let (d_send, d_recv) = unbounded();
        // Drone 12
        let (d12_send, d12_recv) = unbounded();
        // Drone 13
        let (d13_send, d13_recv) = unbounded();
        // Drone 14
        let (d14_send, d14_recv) = unbounded();
        // SC - needed to not make the drone crash
        let (_d_command_send, d_command_recv) = unbounded();
        let (s_event_send, _) = unbounded();
        let (_b, s1_command_recv) = unbounded();
        let (_a, s2_command_recv) = unbounded();

        // Drone 11
        let neighbours11: HashMap<NodeId, Sender<Packet>> = HashMap::from([
            (12, d12_send.clone()),
            (13, d13_send.clone()),
            (14, d14_send.clone()),
            (1, s_send.clone()),
        ]);
        let mut drone11: CppEnjoyersDrone = CppEnjoyersDrone::new(
            11,
            unbounded().0,
            d_command_recv.clone(),
            d_recv.clone(),
            neighbours11,
            0.0,
        );
        // Drone 12
        let neighbours12: HashMap<NodeId, Sender<Packet>> =
            HashMap::from([(1, s_send.clone()), (11, d_send.clone())]);
        let mut drone12: CppEnjoyersDrone = CppEnjoyersDrone::new(
            12,
            unbounded().0,
            d_command_recv.clone(),
            d12_recv.clone(),
            neighbours12,
            0.0,
        );
        // Drone 13
        let neighbours13: HashMap<NodeId, Sender<Packet>> = HashMap::from([
            (11, d_send.clone()),
            (14, d14_send.clone()),
            (2, s2_send.clone()),
        ]);
        let mut drone13: CppEnjoyersDrone = CppEnjoyersDrone::new(
            13,
            unbounded().0,
            d_command_recv.clone(),
            d13_recv.clone(),
            neighbours13,
            0.0,
        );
        // Drone 14
        let neighbours14: HashMap<NodeId, Sender<Packet>> = HashMap::from([
            (11, d_send.clone()),
            (13, d13_send.clone()),
            (2, s2_send.clone()),
        ]);
        let mut drone14: CppEnjoyersDrone = CppEnjoyersDrone::new(
            14,
            unbounded().0,
            d_command_recv.clone(),
            d14_recv.clone(),
            neighbours14,
            0.0,
        );

        // client 1
        let neighbours1: HashMap<u8, Sender<Packet>> =
            HashMap::from([(11, d_send.clone()), (12, d12_send.clone())]);
        let mut server1: GenericServer<Text> = GenericServer::new(
            1,
            s_event_send.clone(),
            s1_command_recv,
            s_recv.clone(),
            neighbours1,
        );

        // server 2
        let neighbours2: HashMap<u8, Sender<Packet>> =
            HashMap::from([(13, d13_send.clone()), (14, d14_send.clone())]);
        let mut server2: GenericServer<Text> = GenericServer::new(
            2,
            s_event_send.clone(),
            s2_command_recv,
            s2_recv.clone(),
            neighbours2,
        );

        // Spawn the drone's run method in a separate thread
        thread::spawn(move || {
            drone11.run();
        });

        thread::spawn(move || {
            drone12.run();
        });

        thread::spawn(move || {
            drone13.run();
        });

        thread::spawn(move || {
            drone14.run();
        });

        thread::spawn(move || {
            server2.run();
        });

        server1.flood();
        while let Ok(p) = server1.packet_recv.recv_timeout(Duration::from_secs(1)) {
            match p.pack_type {
                PacketType::FloodResponse(_) | PacketType::FloodRequest(_) => {
                    server1.handle_packet(p);
                }
                _ => panic!(),
            }
        }

        assert!(graphmap_eq(
            &server1.network_graph.get_graph(),
            &NetworkGraph::from_edges([
                (1, 12, INITIAL_ETX),
                (1, 11, INITIAL_ETX),
                (12, 11, INITIAL_ETX),
                (11, 12, INITIAL_ETX),
                (11, 13, INITIAL_ETX),
                (13, 11, INITIAL_ETX),
                (11, 14, INITIAL_ETX),
                (14, 11, INITIAL_ETX),
                (14, 13, INITIAL_ETX),
                (13, 14, INITIAL_ETX),
                (13, 2, INITIAL_ETX),
                (14, 2, INITIAL_ETX),
            ])
        ));
    }

    /// tests correct flooding behaviour in the event of disconnected graph due to [NackType::ErrorInRouting]
    #[test]
    fn test_flood_server_isolated() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 2, 0, 1, [0; 128]));
        let (ds, dr) = crossbeam_channel::unbounded();
        let (ss, sr) = crossbeam_channel::unbounded();
        let (_, ctrlr) = crossbeam_channel::unbounded();
        server.controller_recv = ctrlr.clone();
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::UnexpectedRecipient(34),
        };
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
        assert!(server.need_flood);
        server.flood();
        assert!(!server.need_flood);
        server.packet_recv = sr.clone();
        assert!(graphmap_eq(
            &server.network_graph.get_graph(),
            &NetworkGraph::from_edges::<[(u8, u8, f64); 0]>([])
        ));
        let neighbours1: HashMap<u8, Sender<Packet>> = HashMap::from([(0, ss.clone())]);
        let mut drone14: CppEnjoyersDrone = CppEnjoyersDrone::new(
            1,
            unbounded().0,
            unbounded().1,
            dr.clone(),
            neighbours1,
            0.0,
        );
        server.network_graph.check_and_add_edge(1, 2);
        thread::spawn(move || {
            drone14.run();
        });
        let cmd: ServerCommand = ServerCommand::AddSender(1, ds.clone());
        server.handle_command(cmd);
        assert!(server.need_flood);
        thread::spawn(move || {
            server.run();
        });
        if let Ok(p) = dr.recv() {
            assert!(p.session_id == 0);
        } else {
            panic!();
        }
    }
}
