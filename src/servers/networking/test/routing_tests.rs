use petgraph::Graph;
use wg_2024::{
    network::SourceRoutingHeader,
    packet::{FloodResponse, NodeType},
};

use crate::servers::{
    self,
    networking::{routing::RoutingTable, test::graphmap_eq},
    test_utils::get_dummy_server_text,
    GenericServer, NetworkGraph, Text, INITIAL_ETX, INITIAL_PDR,
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
