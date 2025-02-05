mod routing_table_test {
    use std::collections::HashMap;

    use crate::servers::{
        networking::routing::{PdrEntry, PdrEstimator, RoutingTable},
        NetworkGraph, INITIAL_ETX, INITIAL_PDR,
    };

    fn get_dummy_graph() -> NetworkGraph {
        NetworkGraph::from_edges([(1, 2, 1.), (1, 3, 1.), (2, 3, 1.), (3, 1, 1.)])
    }

    #[test]
    fn test_new_with_graph() {
        let graph: NetworkGraph = get_dummy_graph();
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let table: RoutingTable = RoutingTable::new_with_graph(graph, estimator);
        assert_eq!(
            table.pdr_table,
            HashMap::from_iter(
                [
                    (1, PdrEntry(INITIAL_PDR, 0, 0)),
                    (2, PdrEntry(INITIAL_PDR, 0, 0)),
                    (3, PdrEntry(INITIAL_PDR, 0, 0)),
                ]
                .into_iter()
            )
        );
        for (_, _, w) in table.graph.all_edges() {
            assert!(*w == INITIAL_ETX);
        }
    }

    #[test]
    fn test_add_edge() {
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let mut table: RoutingTable = RoutingTable::new_with_graph(NetworkGraph::new(), estimator);
        table.add_edge(1, 2);
        table.add_edge(2, 1);
        assert!(table.graph.contains_edge(1, 2));
        assert!(table.graph.contains_edge(2, 1));
        assert!(*table.graph.edge_weight(1, 2).unwrap() == INITIAL_ETX);
        assert!(*table.graph.edge_weight(2, 1).unwrap() == INITIAL_ETX);
        assert!(table.pdr_table.contains_key(&1));
        assert!(table.pdr_table.contains_key(&2));
    }

    #[test]
    fn test_remove_node() {
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let mut table: RoutingTable = RoutingTable::new_with_graph(NetworkGraph::new(), estimator);
        table.add_edge(1, 2);
        table.remove_node(1);
        assert!(!table.pdr_table.contains_key(&1));
        assert!(!table.graph.contains_node(1));
        assert!(table.pdr_table.contains_key(&2));
    }

    #[test]
    fn test_contains_edge() {
        let graph: NetworkGraph = get_dummy_graph();
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let table: RoutingTable = RoutingTable::new_with_graph(graph, estimator);
        assert!(table.graph.contains_edge(1, 2) == table.contains_edge(1, 2));
        assert!(table.graph.contains_edge(1, 3) == table.contains_edge(1, 3));
        assert!(table.graph.contains_edge(2, 3) == table.contains_edge(2, 3));
        assert!(table.graph.contains_edge(3, 1) == table.contains_edge(3, 1));
        assert!(table.graph.contains_edge(1, 4) == table.contains_edge(1, 4));
    }

    #[test]
    fn test_update_pdr_table() {
        let estimator: PdrEstimator =
            PdrEstimator::new(10, |_, acks, nacks| acks as f64 / (acks + nacks) as f64);
        let mut table: RoutingTable = RoutingTable::new_with_graph(NetworkGraph::new(), estimator);
        table.add_edge(1, 2);
        assert!(table.pdr_table.get(&1).unwrap().0 == INITIAL_PDR);
        for i in 0..10 {
            table.update_pdr(1, i < 5);
        }
        assert!((table.pdr_table.get(&1).unwrap().0 - 0.5).abs() < 1e-6);
        for i in 0..10 {
            table.update_pdr(1, i < 2);
        }
        assert!((table.pdr_table.get(&1).unwrap().0 - 0.2).abs() < 1e-6);
        for i in 0..10 {
            table.update_pdr(1, i < 8);
        }
        assert!((table.pdr_table.get(&1).unwrap().0 - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_update_pdr_graph() {
        let estimator: PdrEstimator =
            PdrEstimator::new(10, |_, acks, nacks| acks as f64 / (acks + nacks) as f64);
        let mut table: RoutingTable = RoutingTable::new_with_graph(NetworkGraph::new(), estimator);
        table.add_edge(1, 2);
        table.add_edge(2, 1);
        table.add_edge(1, 3);
        assert!(*table.graph.edge_weight(1, 2).unwrap() == INITIAL_ETX);
        assert!(*table.graph.edge_weight(1, 3).unwrap() == INITIAL_ETX);
        assert!(*table.graph.edge_weight(2, 1).unwrap() == INITIAL_ETX);
        for i in 0..10 {
            table.update_pdr(1, i < 2);
        }
        assert!((*table.graph.edge_weight(1, 2).unwrap() - 1. / 0.2).abs() < RoutingTable::EPSILON);
        assert!((*table.graph.edge_weight(1, 3).unwrap() - 1. / 0.2).abs() < RoutingTable::EPSILON);
        assert!(*table.graph.edge_weight(2, 1).unwrap() == INITIAL_ETX);
        for i in 0..10 {
            table.update_pdr(1, i < 8);
        }
        assert!((*table.graph.edge_weight(1, 2).unwrap() - 1. / 0.8).abs() < RoutingTable::EPSILON);
        assert!((*table.graph.edge_weight(1, 3).unwrap() - 1. / 0.8).abs() < RoutingTable::EPSILON);
        assert!(*table.graph.edge_weight(2, 1).unwrap() == INITIAL_ETX);
    }

    #[test]
    fn test_check_and_add_edge() {
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let mut table: RoutingTable = RoutingTable::new_with_graph(NetworkGraph::new(), estimator);
        table.check_and_add_edge(1, 2);
        assert!(table.graph.contains_edge(1, 2));
        assert!(table.pdr_table.contains_key(&1) && table.pdr_table.contains_key(&2));
        *table.graph.edge_weight_mut(1, 2).unwrap() = 1.;
        table.check_and_add_edge(1, 2);
        assert!(*table.graph.edge_weight_mut(1, 2).unwrap() == 1.);
    }

    #[test]
    fn test_infinite_etx() {
        let estimator: PdrEstimator =
            PdrEstimator::new(10, |_, acks, nacks| acks as f64 / (acks + nacks) as f64);
        let mut table: RoutingTable = RoutingTable::new_with_graph(NetworkGraph::new(), estimator);
        table.check_and_add_edge(1, 2);
        for _ in 0..10 {
            table.update_pdr(1, false);
        }
        assert!(*table.graph.edge_weight(1, 2).unwrap() == f64::INFINITY);
        for i in 0..10 {
            table.update_pdr(1, i < 5);
        }
        assert!(*table.graph.edge_weight(1, 2).unwrap() == 2.);
    }

    #[test]
    fn test_get_route() {
        let graph: NetworkGraph = NetworkGraph::from_edges([
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
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let mut table: RoutingTable = RoutingTable::new(estimator);
        table.graph = graph;
        assert_eq!(table.get_route(0, 9).unwrap(), vec![0, 2, 3, 5, 6, 8, 9]);
        assert!(table.get_route(0, 43).is_none());
    }

    #[test]
    fn test_get_route_infinite_etx() {
        let graph: NetworkGraph = NetworkGraph::from_edges([
            (0, 1, 4.),
            (0, 2, 1.),
            (1, 2, 2.),
            (1, 3, 5.),
            (2, 3, 8.),
            (2, 4, 10.),
            (3, 5, f64::INFINITY),
            (4, 5, 6.),
            (4, 6, 3.),
            (5, 6, 1.),
            (5, 7, 7.),
            (6, 8, 4.),
            (7, 8, 2.),
            (7, 9, 5.),
            (8, 9, 3.),
        ]);
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let mut table: RoutingTable = RoutingTable::new(estimator);
        table.graph = graph;
        assert_eq!(table.get_route(0, 9).unwrap(), vec![0, 2, 4, 6, 8, 9]);
        assert!(table.get_route(0, 43).is_none());
    }

    #[test]
    fn test_get_route_infinite_cost() {
        let graph: NetworkGraph = NetworkGraph::from_edges([(0, 1, 4.), (1, 2, f64::INFINITY)]);
        let estimator: PdrEstimator = PdrEstimator::new(2, |_, _, _| 0.);
        let mut table: RoutingTable = RoutingTable::new(estimator);
        table.graph = graph;
        assert_eq!(table.get_route(0, 2).unwrap(), vec![0, 1, 2]);
    }
}
