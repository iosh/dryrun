use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    primitives::B256,
};

/// A block header whose hash has been recomputed before engine execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBlock {
    header: Sealed<Header>,
}

impl ResolvedBlock {
    pub fn new(header: Sealed<Header>) -> Self {
        Self { header }
    }

    pub(crate) fn header(&self) -> &Header {
        self.header.inner()
    }

    pub(crate) fn number(&self) -> u64 {
        self.header.number()
    }

    pub fn hash(&self) -> B256 {
        self.header.hash()
    }
}
