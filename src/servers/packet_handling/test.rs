#[cfg(test)]
mod packet_tests {
    use common::slc_commands::ServerEvent;
    use wg_2024::{
        network::SourceRoutingHeader,
        packet::{Ack, Fragment, Nack, NackType, PacketType, FRAGMENT_DSIZE},
    };

    use crate::{
        servers::{
            packet_handling::get_rid, test_utils::get_dummy_server_text, NetworkGraph, Text,
        },
        GenericServer,
    };

    #[test]
    fn test_get_rid() {
        assert_eq!(get_rid(u64::from(u16::MAX) + 1), 0);
        assert_eq!(get_rid(u64::MAX), u16::MAX);
        assert_eq!(get_rid(u64::from(u16::MAX) + 56), 55);
    }

    #[test]
    fn test_ack() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let ack: Ack = Ack { fragment_index: 0 };
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        server.handle_ack(0, &ack);
        assert!(server.sent_history.is_empty());
    }

    #[test]
    fn test_ack_missing() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let ack: Ack = Ack { fragment_index: 0 };
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        server.handle_ack(1, &ack);
        assert!(server.sent_history.len() == 1);
    }

    #[test]
    fn test_nack_to_pending() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        server.handle_nack(0, &nack);
        assert_eq!(server.pending_packets.pop_back().unwrap(), 0);
    }

    #[test]
    fn test_nack_resend() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, (2, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        server.network_graph = NetworkGraph::from_edges([(0, 1, 1.), (1, 2, 1.)]);
        let (ds, dr) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        server.handle_nack(0, &nack);
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

    #[test]
    fn test_nack_routing_error() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::ErrorInRouting(1),
        };
        server.network_graph = NetworkGraph::from_edges([(0, 1, 1.), (1, 2, 1.)]);
        server.handle_nack(0, &nack);
        assert_eq!(server.pending_packets.pop_back().unwrap(), 0);
        assert!(!server.network_graph.contains_node(1));
    }

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
                ServerEvent::ShortCut(p) => match p.pack_type {
                    PacketType::Ack(_) => {}
                    _ => {
                        panic!()
                    }
                },
                ServerEvent::PacketSent(_) => {
                    panic!()
                }
            }
        } else {
            panic!();
        }
    }

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
            match p.pack_type {
                PacketType::Ack(_) => {}
                _ => {
                    panic!()
                }
            }
        } else {
            panic!();
        }
    }

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
