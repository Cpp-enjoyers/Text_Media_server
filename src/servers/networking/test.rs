use common::Server;
use itertools::Itertools;
use petgraph::prelude::GraphMap;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

use crate::servers::GenericServer;

#[must_use]
fn get_dummy_server() -> GenericServer {
    let (ctrl_send, _) = crossbeam_channel::unbounded();
    let (_, ctrl_recv) = crossbeam_channel::unbounded();
    let (_, server_recv) = crossbeam_channel::unbounded();
    let server: GenericServer =
        GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
    server
}

/// compares two graphmaps
fn graphmap_eq<N, E, Ty, Ix>(a: &GraphMap<N, E, Ty, Ix>, b: &GraphMap<N, E, Ty, Ix>) -> bool
where
    N: PartialEq + PartialOrd + Hash + Ord + Copy,
    E: PartialEq + Copy + PartialOrd,
    Ty: petgraph::EdgeType,
    Ix: BuildHasher,
{
    // let a_ns = a.nodes();
    // let b_ns = b.nodes();
    let a_es = a.all_edges().map(|e| (e.0, e.1, *e.2));
    let b_es = b.all_edges().map(|e| ((e.0, e.1, *e.2)));
    a_es.sorted_by(|a, b| a.partial_cmp(b).unwrap())
        .eq(b_es.sorted_by(|a, b| a.partial_cmp(b).unwrap()))
    /*
    for (a, b, c) in a_es.sorted_by(|a, b| a.partial_cmp(b).unwrap()) {
        print!("{a}, {b}, {c} - ");
    }
    println!("\n---");
    for (a, b, c) in b_es.sorted_by(|a, b| a.partial_cmp(b).unwrap()) {
        print!("{a}, {b}, {c} - ");
    }
    println!("\n-----");
    true
     */
}

#[cfg(test)]
mod routing_tests {
    use petgraph::Graph;
    use wg_2024::{
        network::SourceRoutingHeader,
        packet::{FloodResponse, NodeType},
    };

    use crate::servers::{
        networking::test::{get_dummy_server, graphmap_eq},
        GenericServer, NetworkGraph,
    };

    /// compares two graphs
    fn graph_eq<N, E, Ty, Ix>(
        a: &petgraph::Graph<N, E, Ty, Ix>,
        b: &petgraph::Graph<N, E, Ty, Ix>,
    ) -> bool
    where
        N: PartialEq,
        E: PartialEq,
        Ty: petgraph::EdgeType,
        Ix: petgraph::graph::IndexType + PartialEq,
    {
        let a_ns = a.raw_nodes().iter().map(|n| &n.weight);
        let b_ns = b.raw_nodes().iter().map(|n| &n.weight);
        let a_es = a
            .raw_edges()
            .iter()
            .map(|e| (e.source(), e.target(), &e.weight));
        let b_es = b
            .raw_edges()
            .iter()
            .map(|e| (e.source(), e.target(), &e.weight));
        a_ns.eq(b_ns) && a_es.eq(b_es)
    }

    #[test]
    fn add_edge_test1() {
        let mut server: GenericServer = get_dummy_server();
        assert!(server.check_and_add_edge(0, 1));
        assert!(!server.check_and_add_edge(0, 1));
        assert!(server.check_and_add_edge(1, 0));
        assert!(server.check_and_add_edge(1, 2));
        assert!(graph_eq(
            &server.network_graph.into_graph::<u8>(),
            &NetworkGraph::from_edges([(0, 1, 1.), (1, 0, 1.), (1, 2, 1.),]).into_graph()
        ));
    }

    #[test]
    fn add_edge_test2() {
        let mut server: GenericServer = get_dummy_server();
        assert!(server.check_and_add_edge(0, 1));
        *server.network_graph.edge_weight_mut(0, 1).unwrap() = 23.;
        assert!(!server.check_and_add_edge(0, 1));
        assert!(server.check_and_add_edge(1, 0));
        assert!(server.check_and_add_edge(1, 2));
        assert!(graph_eq(
            &server.network_graph.into_graph::<u8>(),
            &NetworkGraph::from_edges([(0, 1, 23.), (1, 0, 1.), (1, 2, 1.),]).into_graph()
        ));
    }

    #[test]
    fn test_update_from_flood() {
        let mut server: GenericServer = get_dummy_server();
        let mut fr = FloodResponse {
            flood_id: 1,
            path_trace: vec![],
        };
        server.update_network_from_flood(&fr);
        assert!(graph_eq(
            &server.network_graph.clone().into_graph(),
            &Graph::new()
        ));
        fr.path_trace = vec![
            (0, NodeType::Server),
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (4, NodeType::Client),
        ];
        server.update_network_from_flood(&fr);
        let mut res = NetworkGraph::from_edges([
            (0, 1, 1.),
            (1, 2, 1.),
            (2, 1, 1.),
            (3, 2, 1.),
            (2, 3, 1.),
            (3, 4, 1.),
        ]);
        assert!(graphmap_eq(&server.network_graph, &res,));
        fr.path_trace = vec![(0, NodeType::Server), (5, NodeType::Drone)];
        res.add_edge(0, 5, 1.);
        server.update_network_from_flood(&fr);
        assert!(graphmap_eq(&server.network_graph, &res,));
    }

    #[test]
    fn test_update_from_hdr() {
        let mut server: GenericServer = get_dummy_server();
        let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
        server.update_network_from_header(&hdr);
        let mut res: NetworkGraph = NetworkGraph::from_edges([
            (3, 1, 1.),
            (3, 4, 1.),
            (4, 3, 1.),
            (5, 4, 1.),
            (4, 5, 1.),
            (0, 5, 1.),
        ]);
        println!("{:?}", server.network_graph);
        println!("{:?}", res);
        assert!(graphmap_eq(&server.network_graph, &res));
        let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 2u8, 0u8], 0);
        server.update_network_from_header(&hdr);
        res.add_edge(0, 2, 1.);
        res.add_edge(2, 1, 1.);
        assert!(graphmap_eq(&server.network_graph, &res));
    }

    #[test]
    fn test_get_path() {
        let mut server: GenericServer = get_dummy_server();
        server.network_graph = NetworkGraph::from_edges([
            (0, 1, 4.),
            (0, 2, 1.),
            (1, 2, 2.),
            (1, 3, 5.),
            (2, 3, 8.),
            (2, 4, 10.),
            (3, 5, 2.),
            (4, 5, 6.),
            (4, 6, 3.),
            (5, 6, 1.),
            (5, 7, 7.),
            (6, 8, 4.),
            (7, 8, 2.),
            (7, 9, 5.),
            (8, 9, 3.),
        ]);
        assert_eq!(server.get_route(9).unwrap(), vec![0, 2, 3, 5, 6, 8, 9]);
        assert!(server.get_route(43).is_none());
    }

    #[test]
    fn test_get_srch_from_graph() {
        let mut server: GenericServer = get_dummy_server();
        let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
        server.network_graph =
            NetworkGraph::from_edges([(3, 1, 1.), (0, 3, 1.), (3, 4, 1.), (4, 3, 1.)]);
        assert_eq!(
            server.get_routing_hdr_with_hint(&hdr, 1).hops,
            vec![0u8, 3u8, 1u8]
        );
    }

    #[test]
    fn test_get_srch_from_srch() {
        let mut server: GenericServer = get_dummy_server();
        let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
        server.network_graph = NetworkGraph::from_edges([(0, 3, 1.), (3, 4, 1.)]);
        assert_eq!(
            server.get_routing_hdr_with_hint(&hdr, 1).hops,
            vec![0u8, 5u8, 4u8, 3u8, 1u8]
        );
    }
}

#[cfg(test)]
mod networking_tests {
    use std::{collections::HashMap, time::Duration};

    use common::{networking::flooder::Flooder, slc_commands::ServerEvent, Server};
    use wg_2024::{network::SourceRoutingHeader, packet::Packet};

    use crate::servers::GenericServer;

    use super::get_dummy_server;

    #[test]
    fn test_flood_buffer() {
        let mut server = get_dummy_server();
        assert!(!server.has_seen_flood((1, 64)));
        server.insert_flood((0, 0));
        assert!(server.has_seen_flood((0, 0)));
        assert!(server.flood_history.contains_key(&0));
        assert!(server.flood_history.get(&0).unwrap().contains(&0));
        server.insert_flood((0, 1));
        assert!(server.flood_history.get(&0).unwrap().contains(&0));
        assert!(server.flood_history.get(&0).unwrap().contains(&1));
    }

    #[test]
    fn test_send_to_controller() {
        let (ctrl_send, ctrl_recv_ev) = crossbeam_channel::unbounded();
        let (_, ctrl_recv) = crossbeam_channel::unbounded();
        let (_, server_recv) = crossbeam_channel::unbounded();
        let server: GenericServer =
            GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
        let dummy_pkt: Packet = Packet::new_ack(SourceRoutingHeader::empty_route(), 0, 0);
        server.send_to_controller(dummy_pkt.clone());
        assert!(ctrl_recv_ev.recv().unwrap() == ServerEvent::PacketSent(dummy_pkt));
        assert!(ctrl_recv_ev.recv_timeout(Duration::from_secs(1)).is_err());
    }

    #[test]
    fn test_flood() {}
}
