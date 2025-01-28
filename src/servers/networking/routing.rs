use itertools::Itertools;
use log::{error, info};
use petgraph::{algo::astar, visit::EdgeRef};
use wg_2024::{
    network::{NodeId, SourceRoutingHeader},
    packet::{FloodResponse, NodeType},
};

use crate::servers::GenericServer;

impl GenericServer {
    // TODO remove if too hard to do ETX
    pub(super) fn check_and_add_edge(&mut self, from: u8, to: u8) -> bool {
        (!self.network_graph.contains_edge(from, to))
            .then(|| self.network_graph.add_edge(from, to, 1.))
            .is_some()
    }

    pub(crate) fn update_network_from_flood(&mut self, fr: &FloodResponse) {
        info!("Updating routing info received from flood response");
        for ((prev_id, prev_type), (next_id, next_type)) in fr.path_trace.iter().tuple_windows() {
            match (prev_type, next_type) {
                (NodeType::Drone, NodeType::Drone) => {
                    self.check_and_add_edge(*prev_id, *next_id);
                    self.check_and_add_edge(*next_id, *prev_id);
                }
                (NodeType::Drone, _) => {
                    self.check_and_add_edge(*prev_id, *next_id);
                }
                (_, NodeType::Drone) => {
                    if *prev_id == self.id {
                        self.check_and_add_edge(*prev_id, *next_id);
                    } else {
                        self.check_and_add_edge(*next_id, *prev_id);
                    }
                }
                (_, _) => {
                    error!("Found a Client/Server connected to another Client/Server in flood response: {fr}");
                }
            }
        }
    }

    pub(crate) fn update_network_from_header(&mut self, srch: &SourceRoutingHeader) {
        info!("Updating routing info from source routing header");
        let sz: usize = srch.hops.len();
        if sz < 3 {
            error!("Found wrong src header o client/server directly connected: {srch}");
            return;
        }
        for (prev_id, next_id) in srch.hops[1..srch.hops.len() - 1].iter().tuple_windows() {
            self.check_and_add_edge(*prev_id, *next_id);
            self.check_and_add_edge(*next_id, *prev_id);
        }
        self.check_and_add_edge(srch.hops[1], srch.hops[0]);
        self.check_and_add_edge(srch.hops[sz - 1], srch.hops[sz - 2]);
    }

    pub(crate) fn get_route(&self, dest: NodeId) -> Option<Vec<u8>> {
        astar(
            &self.network_graph,
            self.id,
            |finish| finish == dest,
            |e| *e.weight(),
            |_| 0.,
        )
        .map(|(_, p)| p)
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
