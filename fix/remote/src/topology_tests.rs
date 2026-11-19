use crate::protocol::topology::{Advertisement, LinkId, Neighbor, Topology};
use alloc::{string::String, vec, vec::Vec};

#[test]
fn routes_preserve_links_and_count_each_worker_once() {
    let first = LinkId {
        initiator: [1; 16],
        sequence: 1,
    };
    let second = LinkId {
        initiator: [1; 16],
        sequence: 2,
    };
    let onward = LinkId {
        initiator: [2; 16],
        sequence: 1,
    };
    let edge = |link, node, name: &str| Neighbor {
        link,
        node: [node; 16],
        name: String::from(name),
    };
    let node = |id, neighbors| Advertisement {
        node: [id; 16],
        sequence: 1,
        neighbors,
    };
    let coordinator = node(1, vec![edge(first, 2, "small"), edge(second, 2, "alias")]);
    let small = node(
        2,
        vec![
            edge(first, 1, "coordinator"),
            edge(second, 1, "upstream"),
            edge(onward, 3, "large"),
        ],
    );
    let large = node(3, vec![edge(onward, 2, "small")]);
    let mut topology = Topology::default();
    for advertisement in [coordinator, small.clone(), large] {
        topology.update(advertisement);
    }
    let routes = topology.routes([1; 16]);
    assert_eq!(routes.len(), 2);
    let distant = routes.iter().find(|route| route.node == [3; 16]).unwrap();
    assert_eq!(distant.name, "alias!large");
    assert_eq!(distant.next, second);

    let mut updated = small;
    updated.sequence += 1;
    updated.neighbors.retain(|edge| edge.link != onward);
    let mut update = Topology::default();
    update.update(updated);
    assert!(topology.merge(&update));
    assert_eq!(
        topology
            .routes([1; 16])
            .iter()
            .map(|route| route.node)
            .collect::<Vec<_>>(),
        vec![[2; 16]]
    );
    assert!(!topology.merge(&update));
}
