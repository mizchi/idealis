use std::{
    collections::{HashSet, VecDeque},
    rc::Rc,
};

/// Exact compact key for a single slot. The runs skip the unused bits of the
/// fixed relation grid, so every legal value has a distinct bitmap position.
pub(crate) struct BitmapLayout {
    runs: Vec<(u64, u32)>,
    words: usize,
}

impl BitmapLayout {
    pub(crate) fn new(tuples: &[u64]) -> Option<Self> {
        let mut mask = tuples.iter().fold(0, |mask, tuple| mask | tuple);
        let width = mask.count_ones();
        if width > 16 {
            return None;
        }
        let mut runs = Vec::new();
        let mut packed = 0;
        while mask != 0 {
            let offset = mask.trailing_zeros();
            let length = (mask >> offset).trailing_ones();
            let run = ((1 << length) - 1) << offset;
            runs.push((run, offset - packed));
            mask ^= run;
            packed += length;
        }
        Some(Self {
            runs,
            words: (1usize << width).div_ceil(64),
        })
    }

    fn index(&self, value: u64) -> usize {
        let part = |(mask, shift): &(u64, u32)| (value & mask) >> shift;
        (match self.runs.as_slice() {
            [] => 0,
            [a] => part(a),
            [a, b] => part(a) | part(b),
            [a, b, c, d] => part(a) | part(b) | part(c) | part(d),
            runs => runs.iter().fold(0, |key, run| key | part(run)),
        }) as usize
    }
}

pub(crate) enum SingleSeen<'a> {
    Bitmap(&'a BitmapLayout, Vec<u64>),
    Hash(HashSet<u64>),
}

impl SingleSeen<'_> {
    fn insert(&mut self, value: u64) -> bool {
        match self {
            Self::Bitmap(layout, words) => {
                let index = layout.index(value);
                let bit = 1 << (index % 64);
                let word = &mut words[index / 64];
                if *word & bit != 0 {
                    false
                } else {
                    *word |= bit;
                    true
                }
            }
            Self::Hash(values) => values.insert(value),
        }
    }
}

pub(crate) enum Snapshot {
    Single(u64),
    Pair([u64; 2]),
    Shared(Rc<[u64]>),
}

impl Snapshot {
    pub(crate) fn words(&self) -> &[u64] {
        match self {
            Self::Single(word) => std::slice::from_ref(word),
            Self::Pair(words) => words,
            Self::Shared(words) => words,
        }
    }
}

pub(crate) enum Frontier<'a> {
    Single {
        seen: SingleSeen<'a>,
        queue: VecDeque<(u64, usize)>,
    },
    Pair {
        seen: HashSet<[u64; 2]>,
        queue: VecDeque<([u64; 2], usize)>,
    },
    Shared {
        seen: HashSet<Rc<[u64]>>,
        queue: VecDeque<(Rc<[u64]>, usize)>,
    },
}

impl<'a> Frontier<'a> {
    pub(crate) fn new(slots: usize, layout: Option<&'a BitmapLayout>, budget: usize) -> Self {
        if slots == 1 {
            let seen = match layout.filter(|layout| layout.words <= budget) {
                Some(layout) => SingleSeen::Bitmap(layout, vec![0; layout.words]),
                None => SingleSeen::Hash(HashSet::new()),
            };
            Self::Single {
                seen,
                queue: VecDeque::new(),
            }
        } else if slots == 2 {
            Self::Pair {
                seen: HashSet::new(),
                queue: VecDeque::new(),
            }
        } else {
            Self::Shared {
                seen: HashSet::new(),
                queue: VecDeque::new(),
            }
        }
    }

    pub(crate) fn push(&mut self, words: &[u64], depth: usize) {
        match self {
            Self::Single { seen, queue } => {
                if seen.insert(words[0]) {
                    queue.push_back((words[0], depth));
                }
            }
            Self::Pair { seen, queue } => {
                let value = [words[0], words[1]];
                if seen.insert(value) {
                    queue.push_back((value, depth));
                }
            }
            Self::Shared { seen, queue } => {
                if !seen.contains(words) {
                    let state = Rc::<[u64]>::from(words);
                    seen.insert(state.clone());
                    queue.push_back((state, depth));
                }
            }
        }
    }

    pub(crate) fn pop(&mut self) -> Option<(Snapshot, usize)> {
        match self {
            Self::Single { queue, .. } => queue
                .pop_front()
                .map(|(value, depth)| (Snapshot::Single(value), depth)),
            Self::Pair { queue, .. } => queue
                .pop_front()
                .map(|(value, depth)| (Snapshot::Pair(value), depth)),
            Self::Shared { queue, .. } => queue
                .pop_front()
                .map(|(value, depth)| (Snapshot::Shared(value), depth)),
        }
    }
}
