// Dependency bookkeeping, allows for tasks to be represented as ID's alternative
// to the wait while helping function call in parallel eval, 
// Workers run ready steps without waiting for child tasks.
extern crate alloc;
use alloc::{collections::VecDeque, vec::Vec};

pub enum Transition<A, V> {
    Complete(V),
    Continue(A, Vec<V>),
    Fork(Vec<A>, A),
}
struct Node<A, V> {
    action: Option<A>,
    inputs: Vec<Option<V>>,
    parent: Option<(usize, usize)>,
    waiting: usize,
    running: bool,
}
pub struct TaskGraph<A, V> {
    nodes: Vec<Option<Node<A, V>>>,
    free: Vec<usize>,
    ready: VecDeque<usize>,
    result: Option<V>,
    live: usize,
    in_flight: usize,
}
impl<A, V> TaskGraph<A, V> {
    pub fn new(root: A) -> Self {
        let mut graph = Self {
            nodes: Vec::new(),
            free: Vec::new(),
            ready: VecDeque::new(),
            result: None,
            live: 0,
            in_flight: 0,
        };
        graph.insert(root, None);
        graph
    }
    fn insert(&mut self, action: A, parent: Option<(usize, usize)>) -> usize {
        let node = Some(Node {
            action: Some(action),
            inputs: Vec::new(),
            parent,
            waiting: 0,
            running: false,
        });
        let id = if let Some(id) = self.free.pop() {
            self.nodes[id] = node;
            id
        } else {
            self.nodes.push(node);
            self.nodes.len() - 1
        };
        self.live += 1;
        self.ready.push_back(id);
        id
    }
    pub fn next_ready(&mut self) -> Option<usize> {
        self.ready.pop_front()
    }
    pub fn put_ready_back(&mut self, id: usize) {
        self.ready.push_front(id);
    }
    pub fn start(&mut self, id: usize) -> Option<(A, Vec<V>)> {
        let node = self.nodes.get_mut(id)?.as_mut()?;
        if node.running || node.waiting != 0 {
            return None;
        }
        let action = node.action.take()?;
        node.running = true;
        self.in_flight += 1;
        Some((
            action,
            core::mem::take(&mut node.inputs)
                .into_iter()
                .map(Option::unwrap)
                .collect(),
        ))
    }
    pub fn finish(&mut self, id: usize, transition: Transition<A, V>) {
        assert!(self.nodes[id].as_ref().unwrap().running);
        self.in_flight -= 1;
        match transition {
            Transition::Complete(value) => {
                let node = self.nodes[id].take().unwrap();
                self.free.push(id);
                self.live -= 1;
                if let Some((parent, index)) = node.parent {
                    let parent_node = self.nodes[parent].as_mut().unwrap();
                    assert!(parent_node.inputs[index].is_none());
                    parent_node.inputs[index] = Some(value);
                    parent_node.waiting -= 1;
                    if parent_node.waiting == 0 {
                        self.ready.push_back(parent);
                    }
                } else {
                    assert_eq!(self.live, 0);
                    self.result = Some(value);
                }
            }
            Transition::Continue(action, inputs) => {
                let node = self.nodes[id].as_mut().unwrap();
                node.action = Some(action);
                node.inputs = inputs.into_iter().map(Some).collect();
                node.running = false;
                self.ready.push_back(id);
            }
            Transition::Fork(children, resume) => {
                let node = self.nodes[id].as_mut().unwrap();
                node.action = Some(resume);
                node.running = false;
                node.waiting = children.len();
                node.inputs = core::iter::repeat_with(|| None)
                    .take(children.len())
                    .collect();
                if children.is_empty() {
                    self.ready.push_back(id);
                }
                for (index, action) in children.into_iter().enumerate() {
                    self.insert(action, Some((id, index)));
                }
            }
        }
    }
    pub fn result(&self) -> Option<&V> {
        self.result.as_ref()
    }
    pub fn in_flight(&self) -> usize {
        self.in_flight
    }
    pub fn live(&self) -> usize {
        self.live
    }
}
