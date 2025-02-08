use std::collections::HashMap;

use common::Server;

use super::{GenericServer, Media, Text};

use itertools::Itertools;
use petgraph::prelude::GraphMap;
use std::hash::{BuildHasher, Hash};

/// compares two graphmaps
pub(super) fn graphmap_eq<N, E, Ty, Ix>(
    a: &GraphMap<N, E, Ty, Ix>,
    b: &GraphMap<N, E, Ty, Ix>,
) -> bool
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
}

/// get a minimal [`GenericServer<Text>`]
#[must_use]
pub(super) fn get_dummy_server_text() -> GenericServer<Text> {
    let (ctrl_send, _) = crossbeam_channel::unbounded();
    let (_, ctrl_recv) = crossbeam_channel::unbounded();
    let (_, server_recv) = crossbeam_channel::unbounded();
    let server: GenericServer<Text> =
        GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
    server
}

/// get a minimal [`GenericServer<Media>`]
#[must_use]
pub(super) fn get_dummy_server_media() -> GenericServer<Media> {
    let (ctrl_send, _) = crossbeam_channel::unbounded();
    let (_, ctrl_recv) = crossbeam_channel::unbounded();
    let (_, server_recv) = crossbeam_channel::unbounded();
    let server: GenericServer<Media> =
        GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
    server
}
