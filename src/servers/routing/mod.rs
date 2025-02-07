use std::collections::HashMap;

use itertools::Itertools;
use log::{error, info, warn};
use petgraph::{algo::astar, visit::EdgeRef};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{FloodResponse, NodeType},
};

use crate::servers::{GenericServer, NetworkGraph, ServerType, INITIAL_ETX, INITIAL_PDR};

#[cfg(test)]
mod test;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
struct PdrEntry(f64, u32, u32);

#[derive(Debug, Clone)]
pub(crate) struct PdrEstimator {
    window_sz: u32,
    estimator: fn(old: f64, acks: u32, nacks: u32) -> f64,
}

impl PdrEstimator {
    #[inline]
    #[must_use]
    pub(crate) fn new(
        window_sz: u32,
        estimator: fn(old: f64, acks: u32, nacks: u32) -> f64,
    ) -> Self {
        Self {
            window_sz,
            estimator,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RoutingTable /* <const WINDOW_SIZE: u8> */ {
    graph: NetworkGraph,
    pdr_table: HashMap<NodeId, PdrEntry>,
    pdr_estimator: PdrEstimator,
}

impl RoutingTable {
    const EPSILON: f64 = 1e-3;

    #[inline]
    #[must_use]
    pub(crate) fn new(pdr_estimator: PdrEstimator) -> Self {
        Self {
            graph: NetworkGraph::new(),
            pdr_table: HashMap::new(),
            pdr_estimator,
        }
    }

    #[must_use]
    pub(crate) fn new_with_graph(mut graph: NetworkGraph, pdr_estimator: PdrEstimator) -> Self {
        let it = graph.nodes().map(|n: u8| (n, PdrEntry(INITIAL_PDR, 0, 0)));
        let pdr_table: HashMap<NodeId, PdrEntry> = it.collect();
        // guarantee consistency
        for (_, _, w) in graph.all_edges_mut() {
            *w = INITIAL_ETX;
        }
        Self {
            graph,
            pdr_table,
            pdr_estimator,
        }
    }

    #[inline]
    #[cfg(test)]
    pub(crate) fn get_graph(&self) -> &NetworkGraph {
        &self.graph
    }

    #[inline]
    fn contains_edge(&self, from: NodeId, to: NodeId) -> bool {
        self.graph.contains_edge(from, to)
    }

    fn add_edge(&mut self, from: NodeId, to: NodeId) -> Option<f64> {
        self.pdr_table
            .entry(from)
            .or_insert(PdrEntry(INITIAL_PDR, 0, 0));
        self.pdr_table
            .entry(to)
            .or_insert(PdrEntry(INITIAL_PDR, 0, 0));
        self.graph.add_edge(from, to, INITIAL_ETX)
    }

    pub(crate) fn check_and_add_edge(&mut self, from: NodeId, to: NodeId) -> bool {
        (!self.contains_edge(from, to))
            .then(|| self.add_edge(from, to))
            .is_some()
    }

    pub(super) fn update_pdr(&mut self, id: NodeId, recv: bool) -> bool {
        if self.pdr_table.contains_key(&id) {
            let entry: &mut PdrEntry = self.pdr_table.get_mut(&id).unwrap();
            // update count
            if recv {
                entry.1 += 1;
            } else {
                entry.2 += 1;
            }

            // update pdr and etx, if needed
            if entry.1 + entry.2 == self.pdr_estimator.window_sz {
                entry.0 = (self.pdr_estimator.estimator)(entry.0, entry.1, entry.2);
                entry.1 = 0;
                entry.2 = 0;
                let etx: f64 = if entry.0 < Self::EPSILON {
                    f64::INFINITY
                } else {
                    1. / entry.0
                };
                for (from, _, w) in self.graph.all_edges_mut() {
                    if from == id {
                        *w = etx;
                    }
                }
            }
            true
        } else {
            false
        }
    }

    pub(super) fn get_route(&self, start: NodeId, dest: NodeId) -> Option<Vec<NodeId>> {
        astar(
            &self.graph,
            start,
            |finish: u8| finish == dest,
            |e: (u8, u8, &f64)| *e.weight(),
            |_| 0.,
        )
        .map(|(_, path)| path)
    }

    #[inline]
    pub(crate) fn remove_node(&mut self, id: NodeId) -> bool {
        self.pdr_table.remove(&id);
        self.graph.remove_node(id)
    }
}

impl<T: ServerType> GenericServer<T> {
    #[inline]
    pub(crate) fn check_and_add_edge(&mut self, from: u8, to: u8) -> bool {
        self.network_graph.check_and_add_edge(from, to)
    }

    pub(crate) fn update_pdr_from_ack(&mut self, hops: &[u8]) {
        if hops.len() < 3 {
            warn!(target: &self.target_topic, "warning, received valid ack with invalid routing header, skipping pdr update...");
            return;
        }

        for &id in &hops[1..hops.len() - 1] {
            self.network_graph.update_pdr(id, true);
        }
    }

    pub(crate) fn update_pdr_from_nack(&mut self, hops: &[u8]) {
        if hops.len() < 2 {
            warn!(target: &self.target_topic, "warning, received valid nack with invalid routing header, skipping pdr update...");
            return;
        }

        self.network_graph.update_pdr(hops[0], false);
        for &id in &hops[1..hops.len() - 1] {
            self.network_graph.update_pdr(id, true);
        }
    }

    pub(crate) fn update_network_from_flood(&mut self, fr: &FloodResponse) {
        for ((prev_id, prev_type), (next_id, next_type)) in fr.path_trace.iter().tuple_windows() {
            match (prev_type, next_type) {
                (NodeType::Drone, NodeType::Drone) => {
                    self.check_and_add_edge(*prev_id, *next_id);
                    self.check_and_add_edge(*next_id, *prev_id);
                }
                (NodeType::Drone, _) => {
                    if *next_id == self.id {
                        self.check_and_add_edge(*next_id, *prev_id);
                    } else {
                        self.check_and_add_edge(*prev_id, *next_id);
                    }
                }
                (_, NodeType::Drone) => {
                    if *prev_id == self.id {
                        self.check_and_add_edge(*prev_id, *next_id);
                    } else {
                        self.check_and_add_edge(*next_id, *prev_id);
                    }
                }
                (_, _) => {
                    error!(target: &self.target_topic, "Found a Client/Server connected to another Client/Server in flood response: {fr}");
                }
            }
        }
    }

    pub(crate) fn update_network_from_header(&mut self, srch: &SourceRoutingHeader) {
        info!("Updating routing info from source routing header");
        let sz: usize = srch.hops.len();
        if sz < 3 {
            error!(target: &self.target_topic, "Found wrong src header o client/server directly connected: {srch}");
            return;
        }
        for (prev_id, next_id) in srch.hops[1..srch.hops.len() - 1].iter().tuple_windows() {
            self.check_and_add_edge(*prev_id, *next_id);
            self.check_and_add_edge(*next_id, *prev_id);
        }
        self.check_and_add_edge(srch.hops[1], srch.hops[0]);
        self.check_and_add_edge(srch.hops[sz - 1], srch.hops[sz - 2]);
    }

    pub(crate) fn get_route(&self, dest: NodeId) -> Option<Vec<NodeId>> {
        self.network_graph.get_route(self.id, dest)
    }

    pub(crate) fn get_routing_hdr_with_hint(
        &mut self,
        srch: &SourceRoutingHeader,
        src_id: NodeId,
    ) -> SourceRoutingHeader {
        if let Some(p) = self.get_route(src_id) {
            SourceRoutingHeader::initialize(p)
        } else {
            self.update_network_from_header(srch);
            let mut resp_hdr: SourceRoutingHeader = srch.clone();
            resp_hdr.reverse();
            resp_hdr
        }
    }
}
