use std::iter;

use super::ReverseGraph;

#[test]
fn fixed_ring_and_trap_bound_storage_and_reverse_edge_work() {
    let mut graph = ReverseGraph::default();
    for (from, to) in (0..64).zip((1..64).chain(iter::once(0))) {
        graph.record(from, to).unwrap();
    }
    graph.record(0, 64).unwrap();
    graph.record(0, 65).unwrap();
    graph.record(65, 65).unwrap();
    graph.complete(66);
    // Fixed input measured at 66 nodes, 67 stored edges, 65 reverse edge visits.
    assert_eq!(graph.predecessors.len(), 66);
    assert_eq!(graph.predecessors.iter().map(Vec::len).sum::<usize>(), 67);
    let mut inspected = 0_usize;
    let routes = graph
        .routes_from([64, 64], |_, _| {
            inspected = inspected.checked_add(1).unwrap();
        })
        .unwrap();
    assert_eq!(inspected, 65);
    assert_eq!(routes.len(), 66);
    for (id, has_route) in routes.into_iter().enumerate() {
        assert_eq!(has_route, id != 65);
    }
}
