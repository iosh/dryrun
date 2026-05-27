use cfx_statedb::StateDb;
use cfx_storage::StorageStateTrait;

pub fn new_state_db(storage: Box<dyn StorageStateTrait>) -> StateDb {
    StateDb::new(storage)
}

#[cfg(test)]
mod tests {
    use cfx_types::Address;
    use primitives::{StorageKey, StorageKeyWithSpace};

    #[test]
    fn can_construct_storage_key_with_space() {
        let address = Address::zero();
        let key: StorageKeyWithSpace<'_> = StorageKey::new_account_key(&address).with_evm_space();

        assert!(!key.to_key_bytes().is_empty());
    }
}
