//! Lazily hashed normalized source (plan 0077).
//!
//! A context's normalized source is its parsed commands, each rendered and
//! followed by a newline, then one `(assert ...)` line per later assertion.
//! Each assertion links to its parent in O(1); the SHA-256 state is computed
//! only when a cache key needs it, starting from the nearest ancestor whose
//! state is already known.
use crate::parser::{self, Node};
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub(crate) struct SourceHash(Arc<Link>);
struct Link {
    parent: Option<Arc<Link>>,
    item: Item,
    state: OnceLock<Sha256>,
}
enum Item {
    Commands(Arc<Vec<Node>>),
    Assert(Arc<Node>),
}
impl SourceHash {
    pub(crate) fn commands(nodes: Arc<Vec<Node>>) -> Self {
        Self(Arc::new(Link {
            parent: None,
            item: Item::Commands(nodes),
            state: OnceLock::new(),
        }))
    }
    pub(crate) fn assert(&self, node: Arc<Node>) -> Self {
        Self(Arc::new(Link {
            parent: Some(self.0.clone()),
            item: Item::Assert(node),
            state: OnceLock::new(),
        }))
    }
    /// The running hash state after every command of this source.
    pub(crate) fn state(&self) -> Sha256 {
        if let Some(state) = self.0.state.get() {
            return state.clone();
        }
        let mut pending = vec![&self.0];
        let mut hasher = loop {
            let link = *pending.last().expect("a link");
            match &link.parent {
                Some(parent) => match parent.state.get() {
                    Some(state) => break state.clone(),
                    None => pending.push(parent),
                },
                None => break Sha256::new(),
            }
        };
        for link in pending.iter().rev() {
            match &link.item {
                Item::Commands(nodes) => {
                    for node in nodes.iter() {
                        parser::feed(node, &mut hasher);
                        hasher.update(b"\n");
                    }
                }
                Item::Assert(node) => {
                    hasher.update(b"(assert ");
                    parser::feed(node, &mut hasher);
                    hasher.update(b")\n");
                }
            }
        }
        self.0.state.get_or_init(|| hasher).clone()
    }
    pub(crate) fn digest(&self) -> [u8; 32] {
        self.state().finalize().into()
    }
}
impl Drop for Link {
    /// Unlinks iteratively so that a long assertion chain cannot overflow
    /// the stack when it is dropped.
    fn drop(&mut self) {
        let mut next = self.parent.take();
        while let Some(link) = next {
            next = match Arc::try_unwrap(link) {
                Ok(mut owned) => owned.parent.take(),
                Err(_) => None,
            };
        }
    }
}
