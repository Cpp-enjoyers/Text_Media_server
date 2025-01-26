use itertools::Itertools;
use log::error;
use wg_2024::{
    network::SourceRoutingHeader,
    packet::{FloodResponse, NodeType},
};

use crate::servers::GenericServer;

impl GenericServer {
    // TODO remove if too hard to do ETX
    fn check_and_add_edge(&mut self, from: u8, to: u8) -> bool {
        (!self.network_graph.contains_edge(from, to))
            .then(|| self.network_graph.add_edge(from, to, 1.))
            .is_some()
    }

    pub(crate) fn update_network_from_flood(&mut self, fr: &FloodResponse) {
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
                    self.check_and_add_edge(*next_id, *prev_id);
                }
                (_, _) => {
                    error!("Found a Client/Server connected to another Client/Server in flood response: {fr}");
                }
            }
        }
    }

    pub(crate) fn update_network_from_header(&mut self, srch: &mut SourceRoutingHeader) {
        if srch.hops.len() < 2 {
            return;
        }
        let lst: u8 = srch.hops.pop().unwrap();
        let fst: u8 = srch.hops.remove(0);
        for (prev_id, next_id) in srch.hops.iter().tuple_windows() {
            self.check_and_add_edge(*prev_id, *next_id);
            self.check_and_add_edge(*next_id, *prev_id);
        }
        if let Some(d_fst) = srch.hops.first() {
            self.check_and_add_edge(*d_fst, fst);
        }
        if let Some(d_lst) = srch.hops.last() {
            self.check_and_add_edge(lst, *d_lst);
        }
    }
}
