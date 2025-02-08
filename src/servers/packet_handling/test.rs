#[cfg(test)]
mod packet_tests {
    use common::slc_commands::ServerEvent;
    use wg_2024::{
        network::SourceRoutingHeader,
        packet::{Ack, Fragment, Nack, NackType, PacketType, FRAGMENT_DSIZE},
    };

    use crate::{
        servers::{
            self, routing::RoutingTable, test_utils::get_dummy_server_text, HistoryEntry,
            NetworkGraph, Text, INITIAL_PDR,
        },
        GenericServer,
    };

    /// tests correct [Ack] handling
    #[test]
    fn test_ack() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let ack: Ack = Ack { fragment_index: 0 };
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 1, 0, 1, [0; FRAGMENT_DSIZE]));
        server.handle_ack(0, &ack);
        assert!(server.sent_history.is_empty());
    }

    #[test]
    fn test_ack_missing() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let ack: Ack = Ack { fragment_index: 0 };
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 1, 0, 1, [0; FRAGMENT_DSIZE]));
        server.handle_ack(1, &ack);
        assert!(server.sent_history.len() == 1);
    }

    /// tests correct [Nack] to pending behaviour 
    #[test]
    fn test_nack_to_pending() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 1, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        server.handle_nack(0, &SourceRoutingHeader::new(vec![1, 0], 0), &nack);
        assert_eq!(server.pending_packets.pop_back().unwrap(), 0);
    }

    /// tests correct [Nack] resend behaviour
    #[test]
    fn test_nack_resend() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 2, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        server.network_graph = RoutingTable::new_with_graph(
            NetworkGraph::from_edges([(0, 1, INITIAL_PDR), (1, 2, INITIAL_PDR)]),
            servers::default_estimator(),
        );
        let (ds, dr) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
        if let Ok(packet) = dr.recv() {
            match packet.pack_type {
                PacketType::MsgFragment(f) => {
                    assert_eq!(f.data, [0; FRAGMENT_DSIZE]);
                }
                _ => {
                    panic!();
                }
            }
        } else {
            panic!()
        }
    }

    /// test [Nack] behaviour in the event of multiple ones
    #[test]
    fn test_nack_resend_trice() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 2, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        server.network_graph = RoutingTable::new_with_graph(
            NetworkGraph::from_edges([(0, 1, INITIAL_PDR), (1, 2, INITIAL_PDR)]),
            servers::default_estimator(),
        );
        let (ds, dr) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
        for _ in 0..3 {
            if let Ok(packet) = dr.recv() {
                match packet.pack_type {
                    PacketType::MsgFragment(f) => {
                        assert_eq!(f.data, [0; FRAGMENT_DSIZE]);
                    }
                    _ => {
                        panic!();
                    }
                }
            } else {
                panic!()
            }
        }
        assert!(dr.try_recv().is_err());
    }

    /// test graph consistency update in the event of a [NackType::ErrorInRouting]
    #[test]
    fn test_nack_routing_error() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, HistoryEntry::new(vec![], 1, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::ErrorInRouting(1),
        };
        server.network_graph = RoutingTable::new_with_graph(
            NetworkGraph::from_edges([(0, 1, INITIAL_PDR), (1, 2, INITIAL_PDR)]),
            servers::default_estimator(),
        );
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
        assert_eq!(server.pending_packets.pop_back().unwrap(), 0);
        assert!(!server.network_graph.get_graph().contains_node(1));
    }

    /// tests correct [Ack] shortcutting in case of missing route
    #[test]
    fn test_fragment_recv_scl_ack() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let (scls, sclr) = crossbeam_channel::unbounded();
        server.controller_send = scls.clone();
        server.handle_fragment(
            &SourceRoutingHeader::new(vec![2, 1, 0], 2),
            0,
            &Fragment {
                fragment_index: 0,
                total_n_fragments: 2,
                length: 11,
                data: [0; 128],
            },
        );
        let (sz, frag) = server.fragment_history.remove(&(2, 0)).unwrap();
        assert!(frag.len() == 2);
        assert!(sz == 1);
        assert!(frag[0] == [0u8; 128]);
        assert!(server.fragment_history.is_empty());
        if let Ok(p) = sclr.recv() {
            match p {
                ServerEvent::ShortCut(p) => {
                    matches!(p.pack_type, PacketType::Ack(_));
                }
                ServerEvent::PacketSent(_) => {
                    panic!();
                }
            }
        } else {
            panic!();
        }
    }

    /// tests correct [Ack] sending in case of existing route
    #[test]
    fn test_fragment_recv_drone_ack() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let (ds, dr) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        server.handle_fragment(
            &SourceRoutingHeader::new(vec![2, 1, 0], 2),
            0,
            &Fragment {
                fragment_index: 0,
                total_n_fragments: 2,
                length: 11,
                data: [0; 128],
            },
        );
        let (sz, frag) = server.fragment_history.remove(&(2, 0)).unwrap();
        assert!(frag.len() == 2);
        assert!(sz == 1);
        assert!(frag[0] == [0u8; 128]);
        assert!(server.fragment_history.is_empty());
        if let Ok(p) = dr.recv() {
            matches!(p.pack_type, PacketType::Ack(_));
        } else {
            panic!();
        }
    }

    /// tests correct handling of ill formed fragments
    #[test]
    fn test_bad_fragment_recv() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server.handle_fragment(
            &SourceRoutingHeader::empty_route(),
            0,
            &Fragment {
                fragment_index: 0,
                total_n_fragments: 2,
                length: 11,
                data: [0; 128],
            },
        );
        assert!(server.fragment_history.is_empty());
    }
}
