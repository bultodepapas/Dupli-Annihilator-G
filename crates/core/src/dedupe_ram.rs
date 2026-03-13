use ahash::RandomState;
use hashbrown::HashSet;
use indexmap::IndexSet;

#[derive(Debug)]
pub enum RamStore {
    Stable(IndexSet<Box<str>, RandomState>),
    Unordered(HashSet<Box<str>, RandomState>),
}

impl RamStore {
    pub fn new_stable() -> Self {
        Self::Stable(IndexSet::with_hasher(RandomState::new()))
    }

    pub fn new_unordered() -> Self {
        Self::Unordered(HashSet::with_hasher(RandomState::new()))
    }

    pub fn reserve(&mut self, n: usize) {
        match self {
            Self::Stable(set) => set.reserve(n),
            Self::Unordered(set) => set.reserve(n),
        }
    }

    pub fn insert(&mut self, token: &str) -> bool {
        match self {
            Self::Stable(set) => set.insert_full(token.into()).1,
            Self::Unordered(set) => set.insert(token.into()),
        }
    }

    pub fn into_tokens(self) -> Vec<Box<str>> {
        match self {
            Self::Stable(set) => set.into_iter().collect(),
            Self::Unordered(set) => set.into_iter().collect(),
        }
    }
}
