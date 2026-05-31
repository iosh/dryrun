use std::sync::Arc;

use cfx_types::{Address, H256, U256};
use keccak_hash::keccak;
use primitives::{CodeInfo, account::EthereumAccount, storage::StorageValue};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum StateValueCodecError {
    #[error("eSpace code hash mismatch: expected {expected:?}, got {actual:?}")]
    EspaceCodeHashMismatch { expected: H256, actual: H256 },
}

// eSpace account values follow the upstream EthereumAccount RLP layout.
pub(crate) fn encode_espace_account(balance: U256, nonce: U256, code_hash: H256) -> Box<[u8]> {
    rlp::encode(&EthereumAccount {
        balance,
        nonce,
        code_hash,
    })
    .to_vec()
    .into_boxed_slice()
}

// eSpace storage slots are encoded as StorageValue with no owner.
pub(crate) fn encode_espace_storage_slot(value: U256) -> Box<[u8]> {
    rlp::encode(&StorageValue { value, owner: None })
        .to_vec()
        .into_boxed_slice()
}

// Native global params are stored as plain U256 RLP values.
pub(crate) fn encode_native_u256(value: U256) -> Box<[u8]> {
    rlp::encode(&value).to_vec().into_boxed_slice()
}

// eSpace code values follow the upstream CodeInfo RLP layout.
// CodeKey already carries the expected hash, so we verify the code bytes first.

pub(crate) fn encode_espace_code(
    expected_code_hash: H256,
    code: Vec<u8>,
) -> Result<Box<[u8]>, StateValueCodecError> {
    let actual_code_hash = keccak(&code);
    if actual_code_hash != expected_code_hash {
        return Err(StateValueCodecError::EspaceCodeHashMismatch {
            expected: expected_code_hash,
            actual: actual_code_hash,
        });
    }

    Ok(rlp::encode(&CodeInfo {
        code: Arc::new(code),
        owner: Address::zero(),
    })
    .to_vec()
    .into_boxed_slice())
}
