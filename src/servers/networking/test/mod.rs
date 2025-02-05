use itertools::Itertools;
use petgraph::prelude::GraphMap;
use std::hash::{BuildHasher, Hash};

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
mod networking_tests;
#[cfg(test)]
mod routing_tests;