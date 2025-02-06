use std::collections::HashMap;

use common::{slc_commands::ServerCommand, Server};
use crossbeam_channel::Sender;
use petgraph::Graph;
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{Ack, FloodResponse, Nack, NackType, NodeType, Packet},
};

use crate::servers::{
    self, networking::{routing::RoutingTable, test::graphmap_eq}, test_utils::get_dummy_server_text, GenericServer, HistoryEntry, NetworkGraph, Text, DEFAULT_WINDOW_SZ, INITIAL_ETX, INITIAL_PDR
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
    let mut server: GenericServer<Text> = get_dummy_server_text();
    assert!(server.check_and_add_edge(0, 1));
    assert!(!server.check_and_add_edge(0, 1));
    assert!(server.check_and_add_edge(1, 0));
    assert!(server.check_and_add_edge(1, 2));
    assert!(graphmap_eq(
        server.network_graph.get_graph(),
        &NetworkGraph::from_edges([
            (0, 1, INITIAL_ETX),
            (1, 0, INITIAL_ETX),
            (1, 2, INITIAL_ETX),
        ])
    ));
}

#[test]
fn test_update_from_flood() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let mut fr = FloodResponse {
        flood_id: 1,
        path_trace: vec![],
    };
    server.update_network_from_flood(&fr);
    assert!(graph_eq(
        &server.network_graph.get_graph().clone().into_graph(),
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
    let mut res: NetworkGraph = NetworkGraph::from_edges([
        (0, 1, INITIAL_ETX),
        (1, 2, INITIAL_ETX),
        (2, 1, INITIAL_ETX),
        (3, 2, INITIAL_ETX),
        (2, 3, INITIAL_ETX),
        (3, 4, INITIAL_ETX),
    ]);
    assert!(graphmap_eq(&server.network_graph.get_graph(), &res,));
    fr.path_trace = vec![(0, NodeType::Server), (5, NodeType::Drone)];
    res.add_edge(0, 5, INITIAL_ETX);
    server.update_network_from_flood(&fr);
    assert!(graphmap_eq(&server.network_graph.get_graph(), &res,));
}

#[test]
fn test_update_from_hdr() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
    server.update_network_from_header(&hdr);
    let mut res: NetworkGraph = NetworkGraph::from_edges([
        (3, 1, INITIAL_ETX),
        (3, 4, INITIAL_ETX),
        (4, 3, INITIAL_ETX),
        (5, 4, INITIAL_ETX),
        (4, 5, INITIAL_ETX),
        (0, 5, INITIAL_ETX),
    ]);
    println!("{:?}", server.network_graph);
    println!("{:?}", res);
    assert!(graphmap_eq(&server.network_graph.get_graph(), &res));
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 2u8, 0u8], 0);
    server.update_network_from_header(&hdr);
    res.add_edge(0, 2, INITIAL_ETX);
    res.add_edge(2, 1, INITIAL_ETX);
    assert!(graphmap_eq(&server.network_graph.get_graph(), &res));
}

#[test]
fn test_get_srch_from_graph() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
    server.network_graph = RoutingTable::new_with_graph(
        NetworkGraph::from_edges([
            (3, 1, INITIAL_PDR),
            (0, 3, INITIAL_PDR),
            (3, 4, INITIAL_PDR),
            (4, 3, INITIAL_PDR),
        ]),
        servers::default_estimator(),
    );
    assert_eq!(
        server.get_routing_hdr_with_hint(&hdr, 1).hops,
        vec![0u8, 3u8, 1u8]
    );
}

#[test]
fn test_get_srch_from_srch() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
    server.network_graph = RoutingTable::new_with_graph(
        NetworkGraph::from_edges([(0, 3, INITIAL_PDR), (3, 4, INITIAL_PDR)]),
        servers::default_estimator(),
    );
    assert_eq!(
        server.get_routing_hdr_with_hint(&hdr, 1).hops,
        vec![0u8, 5u8, 4u8, 3u8, 1u8]
    );
}

#[test]
fn test_graph_consistency() {
    let (ctrls, _) = crossbeam_channel::unbounded();
    let (_, ctrlr) = crossbeam_channel::unbounded();
    let (_, st) = crossbeam_channel::unbounded();
    let mut map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
    let (ds, _) = crossbeam_channel::unbounded();
    map.insert(1, ds.clone());
    let mut server: GenericServer<Text> = <GenericServer<Text>>::new(0, ctrls, ctrlr, st, map);
    let (ds, _) = crossbeam_channel::unbounded();
    let cmd: ServerCommand = ServerCommand::AddSender(2, ds.clone());
    server.handle_command(cmd);
    let srch: SourceRoutingHeader = SourceRoutingHeader::new(vec![5, 4, 3, 0], 3);
    server.update_network_from_header(&srch);
    let fr: FloodResponse = FloodResponse {
        flood_id: 0,
        path_trace: vec![
            (0, NodeType::Server),
            (6, NodeType::Drone),
            (7, NodeType::Client),
            (8, NodeType::Drone),
            (9, NodeType::Drone),
        ],
    };
    server.update_network_from_flood(&fr);
    assert!(graphmap_eq(
        server.network_graph.get_graph(),
        &NetworkGraph::from_edges([
            (0, 1, INITIAL_ETX),
            (0, 2, INITIAL_ETX),
            (4, 3, INITIAL_ETX),
            (3, 4, INITIAL_ETX),
            (0, 3, INITIAL_ETX),
            (4, 5, INITIAL_ETX),
            (0, 6, INITIAL_ETX),
            (6, 7, INITIAL_ETX),
            (8, 7, INITIAL_ETX),
            (8, 9, INITIAL_ETX),
            (9, 8, INITIAL_ETX),
        ])
    ));
}

#[test]
fn test_etx_nacks_update_to_inf_and_back() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let fr: FloodResponse = FloodResponse { flood_id: 0, path_trace: vec![(0, NodeType::Server), (1, NodeType::Drone), (2, NodeType::Drone), (3, NodeType::Client)] };
    server.update_network_from_flood(&fr);
    server.sent_history.insert(0, HistoryEntry { hops: vec![0, 2], receiver_id: 2, frag_idx: 0, n_frags: 1, frag: [0; 128] });
    let nack: Nack = Nack { fragment_index: 0, nack_type: NackType::Dropped };
    let (ds, _) = crossbeam_channel::unbounded();
    let cmd: ServerCommand = ServerCommand::AddSender(1, ds.clone());
    server.handle_command(cmd);
    // to find 15 solve BETA^x * INTIAL_PDR < EPSILON
    for _ in 0..DEFAULT_WINDOW_SZ * 15 {
        server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
    }
    assert!(*server.network_graph.get_graph().edge_weight(1, 2).unwrap() == f64::INFINITY);
    assert!(server.sent_history.get(&0).unwrap().hops == vec![0, 1, 2]);
    server.handle_nack(0, &SourceRoutingHeader::initialize(vec![1, 0]), &nack);
    assert!(server.sent_history.get(&0).unwrap().hops == vec![0, 1, 2]);
    for i in 1..DEFAULT_WINDOW_SZ * 2{
        server.sent_history.insert(i as u64, HistoryEntry { hops: vec![0, 1, 2], receiver_id: 2, frag_idx: 0, n_frags: 1, frag: [0; 128] });
    }
    let ack: Ack = Ack { fragment_index: 0 };
    for i in 0..DEFAULT_WINDOW_SZ * 2{
        server.handle_ack(i as u64, &ack);
    }
    assert!(*server.network_graph.get_graph().edge_weight(1, 2).unwrap() != f64::INFINITY);
}

#[test]
fn test_etx_acks_update_to_1() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let fr: FloodResponse = FloodResponse { flood_id: 0, path_trace: vec![(0, NodeType::Server), (1, NodeType::Drone), (2, NodeType::Drone), (3, NodeType::Client)] };
    server.update_network_from_flood(&fr);
    for i in 0..(DEFAULT_WINDOW_SZ * 15) {
        server.sent_history.insert(i as u64, HistoryEntry { hops: vec![0, 1, 2], receiver_id: 2, frag_idx: 0, n_frags: 1, frag: [0; 128] });
    }
    let ack: Ack = Ack { fragment_index: 0 };
    let (ds, _) = crossbeam_channel::unbounded();
    let cmd: ServerCommand = ServerCommand::AddSender(1, ds.clone());
    server.handle_command(cmd);
    // to find 15 solve BETA^x * INTIAL_PDR < EPSILON (yes, the limit approach is the same as above)
    // just find it at infinity rather than 0 and invert the variable
    for i in 0..DEFAULT_WINDOW_SZ * 15 {
        server.handle_ack(i as u64, &ack);
    }
    assert!((*server.network_graph.get_graph().edge_weight(1, 2).unwrap() - 1.).abs() < 1e-3);
}
