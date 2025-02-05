use petgraph::Graph;
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{FloodResponse, NodeType},
};

use crate::servers::{
    networking::test::graphmap_eq, test_utils::get_dummy_server_text, GenericServer, NetworkGraph,
    Text, INITIAL_PDR,
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
    assert!(graph_eq(
        &server.network_graph.into_graph::<NodeId>(),
        &NetworkGraph::from_edges([
            (0, 1, INITIAL_PDR),
            (1, 0, INITIAL_PDR),
            (1, 2, INITIAL_PDR),
        ])
        .into_graph()
    ));
}

#[test]
fn add_edge_test2() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    assert!(server.check_and_add_edge(0, 1));
    *server.network_graph.edge_weight_mut(0, 1).unwrap() = 23.;
    assert!(!server.check_and_add_edge(0, 1));
    assert!(server.check_and_add_edge(1, 0));
    assert!(server.check_and_add_edge(1, 2));
    assert!(graph_eq(
        &server.network_graph.into_graph::<NodeId>(),
        &NetworkGraph::from_edges([(0, 1, 23.), (1, 0, INITIAL_PDR), (1, 2, INITIAL_PDR),])
            .into_graph()
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
    let mut res: NetworkGraph = NetworkGraph::from_edges([
        (0, 1, INITIAL_PDR),
        (1, 2, INITIAL_PDR),
        (2, 1, INITIAL_PDR),
        (3, 2, INITIAL_PDR),
        (2, 3, INITIAL_PDR),
        (3, 4, INITIAL_PDR),
    ]);
    assert!(graphmap_eq(&server.network_graph, &res,));
    fr.path_trace = vec![(0, NodeType::Server), (5, NodeType::Drone)];
    res.add_edge(0, 5, INITIAL_PDR);
    server.update_network_from_flood(&fr);
    assert!(graphmap_eq(&server.network_graph, &res,));
}

#[test]
fn test_update_from_hdr() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
    server.update_network_from_header(&hdr);
    let mut res: NetworkGraph = NetworkGraph::from_edges([
        (3, 1, INITIAL_PDR),
        (3, 4, INITIAL_PDR),
        (4, 3, INITIAL_PDR),
        (5, 4, INITIAL_PDR),
        (4, 5, INITIAL_PDR),
        (0, 5, INITIAL_PDR),
    ]);
    println!("{:?}", server.network_graph);
    println!("{:?}", res);
    assert!(graphmap_eq(&server.network_graph, &res));
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 2u8, 0u8], 0);
    server.update_network_from_header(&hdr);
    res.add_edge(0, 2, INITIAL_PDR);
    res.add_edge(2, 1, INITIAL_PDR);
    assert!(graphmap_eq(&server.network_graph, &res));
}

#[test]
fn test_get_path() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
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
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
    server.network_graph = NetworkGraph::from_edges([
        (3, 1, INITIAL_PDR),
        (0, 3, INITIAL_PDR),
        (3, 4, INITIAL_PDR),
        (4, 3, INITIAL_PDR),
    ]);
    assert_eq!(
        server.get_routing_hdr_with_hint(&hdr, 1).hops,
        vec![0u8, 3u8, 1u8]
    );
}

#[test]
fn test_get_srch_from_srch() {
    let mut server: GenericServer<Text> = get_dummy_server_text();
    let hdr: SourceRoutingHeader = SourceRoutingHeader::new(vec![1u8, 3u8, 4u8, 5u8, 0u8], 0);
    server.network_graph = NetworkGraph::from_edges([(0, 3, INITIAL_PDR), (3, 4, INITIAL_PDR)]);
    assert_eq!(
        server.get_routing_hdr_with_hint(&hdr, 1).hops,
        vec![0u8, 5u8, 4u8, 3u8, 1u8]
    );
}
