use alloc::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    string::String,
    vec::Vec,
};
use serde::{Deserialize, Serialize};

pub type NodeId = [u8; 16];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LinkId {
    pub initiator: NodeId,
    pub sequence: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Neighbor {
    pub link: LinkId,
    pub node: NodeId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Advertisement {
    pub node: NodeId,
    pub sequence: u64,
    pub neighbors: Vec<Neighbor>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Topology {
    nodes: BTreeMap<NodeId, Advertisement>,
}

#[derive(Clone)]
pub struct Route {
    pub node: NodeId,
    pub next: LinkId,
    pub name: String,
}

impl Topology {
    fn is_newer(&self, advertisement: &Advertisement) -> bool {
        self.nodes
            .get(&advertisement.node)
            .is_none_or(|previous| previous.sequence < advertisement.sequence)
    }

    pub fn update(&mut self, advertisement: Advertisement) -> bool {
        if !self.is_newer(&advertisement) {
            return false;
        }
        self.nodes.insert(advertisement.node, advertisement);
        true
    }

    pub fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for advertisement in other.nodes.values() {
            if self.is_newer(advertisement) {
                self.nodes.insert(advertisement.node, advertisement.clone());
                changed = true;
            }
        }
        changed
    }

    pub fn merge_updates(&mut self, mut updates: Self) -> Option<Self> {
        updates
            .nodes
            .retain(|_, advertisement| self.is_newer(advertisement));
        self.merge(&updates).then_some(updates)
    }

    pub fn routes(&self, origin: NodeId) -> Vec<Route> {
        let mut pending = VecDeque::new();
        let mut seen = BTreeSet::from([origin]);
        let mut routes = Vec::new();
        if let Some(local) = self.nodes.get(&origin) {
            pending.push_back((local, None, String::new()));
        }
        while let Some((source, next, name)) = pending.pop_front() {
            let mut neighbors: Vec<_> = source.neighbors.iter().collect();
            neighbors
                .sort_by_key(|neighbor| (neighbor.name.is_empty(), &neighbor.name, neighbor.link));
            for neighbor in neighbors {
                let Some(target) = self.nodes.get(&neighbor.node) else {
                    continue;
                };
                if !target
                    .neighbors
                    .iter()
                    .any(|reverse| reverse.link == neighbor.link && reverse.node == source.node)
                    || !seen.insert(target.node)
                {
                    continue;
                }
                let next = next.unwrap_or(neighbor.link);
                let segment = if neighbor.name.is_empty() {
                    neighbor
                        .node
                        .iter()
                        .map(|byte| alloc::format!("{byte:02x}"))
                        .collect::<String>()
                } else {
                    neighbor.name.clone()
                };
                let name = if name.is_empty() {
                    segment
                } else {
                    alloc::format!("{name}!{segment}")
                };
                routes.push(Route {
                    node: target.node,
                    next,
                    name: name.clone(),
                });
                pending.push_back((target, Some(next), name));
            }
        }
        routes
    }
}
