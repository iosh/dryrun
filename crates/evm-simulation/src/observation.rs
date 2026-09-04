use alloy::primitives::{Address, B256};

/// A log location whose post-log state must be retained for later verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvmLogCheckpoint {
    address: Option<Address>,
    topic0: B256,
}

impl EvmLogCheckpoint {
    const fn any_address(topic0: B256) -> Self {
        Self {
            address: None,
            topic0,
        }
    }

    const fn at(address: Address, topic0: B256) -> Self {
        Self {
            address: Some(address),
            topic0,
        }
    }

    fn matches(self, address: Address, topic0: &B256) -> bool {
        self.topic0 == *topic0 && self.address.is_none_or(|expected| expected == address)
    }
}

/// State retention requested by a change resolver before execution starts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvmObservationRequirements {
    log_checkpoints: Vec<EvmLogCheckpoint>,
}

impl EvmObservationRequirements {
    pub const fn new() -> Self {
        Self {
            log_checkpoints: Vec::new(),
        }
    }

    pub fn checkpoint_any_address(&mut self, topic0: B256) {
        self.insert(EvmLogCheckpoint::any_address(topic0));
    }

    pub fn checkpoint_at(&mut self, address: Address, topic0: B256) {
        self.insert(EvmLogCheckpoint::at(address, topic0));
    }

    pub(crate) fn merge(mut self, other: &Self) -> Self {
        for checkpoint in &other.log_checkpoints {
            self.insert(*checkpoint);
        }
        self
    }

    pub(crate) fn matches_log(&self, address: Address, topic0: &B256) -> bool {
        self.log_checkpoints
            .iter()
            .any(|checkpoint| checkpoint.matches(address, topic0))
    }

    fn insert(&mut self, checkpoint: EvmLogCheckpoint) {
        if !self.log_checkpoints.contains(&checkpoint) {
            self.log_checkpoints.push(checkpoint);
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256};

    use super::EvmObservationRequirements;

    #[test]
    fn merges_and_deduplicates_log_conditions() {
        let mut first = EvmObservationRequirements::new();
        first.checkpoint_any_address(B256::ZERO);
        first.checkpoint_any_address(B256::ZERO);

        let mut second = EvmObservationRequirements::new();
        second.checkpoint_at(Address::ZERO, B256::ZERO);

        let merged = first.merge(&second);
        assert_eq!(merged.log_checkpoints.len(), 2);
        assert!(merged.matches_log(Address::ZERO, &B256::ZERO));
    }
}
