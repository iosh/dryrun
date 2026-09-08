use alloy::primitives::{Address, U256};
use contract_standards::Erc1155TransferItem;

use crate::EvmStandardChange;

use super::metadata::TokenMetadataOutcomes;

#[derive(Debug)]
pub(super) enum VerifiedTokenEvent {
    Standard(VerifiedTokenChange),
    Wrapped,
}

#[derive(Debug)]
pub(super) enum VerifiedTokenChange {
    Erc20Transfer {
        contract: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Approval {
        contract: Address,
        owner: Address,
        spender: Address,
        before: U256,
        after: U256,
    },
    Erc721Transfer {
        contract: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc721Approval {
        contract: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        token_id: U256,
    },
    OperatorApproval {
        contract: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155TransferBatch {
        contract: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<(U256, U256)>,
    },
}

impl VerifiedTokenChange {
    pub(super) fn into_change(self, metadata: &TokenMetadataOutcomes) -> EvmStandardChange {
        match self {
            Self::Erc20Transfer {
                contract,
                from,
                to,
                amount,
            } => {
                let metadata = metadata.erc20(&contract);
                if from == Address::ZERO {
                    EvmStandardChange::Erc20Mint {
                        contract_address: contract,
                        to,
                        raw_amount: amount,
                        metadata,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc20Burn {
                        contract_address: contract,
                        from,
                        raw_amount: amount,
                        metadata,
                    }
                } else {
                    EvmStandardChange::Erc20Transfer {
                        contract_address: contract,
                        from,
                        to,
                        raw_amount: amount,
                        metadata,
                    }
                }
            }
            Self::Erc20Approval {
                contract,
                owner,
                spender,
                before,
                after,
            } => EvmStandardChange::Erc20Approval {
                contract_address: contract,
                owner,
                spender,
                before,
                after,
                metadata: metadata.erc20(&contract),
            },
            Self::Erc721Transfer {
                contract,
                from,
                to,
                token_id,
            } => {
                let metadata = metadata.erc721(&contract);
                if from == Address::ZERO {
                    EvmStandardChange::Erc721Mint {
                        contract_address: contract,
                        to,
                        token_id,
                        metadata,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc721Burn {
                        contract_address: contract,
                        from,
                        token_id,
                        metadata,
                    }
                } else {
                    EvmStandardChange::Erc721Transfer {
                        contract_address: contract,
                        from,
                        to,
                        token_id,
                        metadata,
                    }
                }
            }
            Self::Erc721Approval {
                contract,
                owner,
                before,
                after,
                token_id,
            } => EvmStandardChange::Erc721Approval {
                contract_address: contract,
                owner,
                before,
                after,
                token_id,
                metadata: metadata.erc721(&contract),
            },
            Self::OperatorApproval {
                contract,
                owner,
                operator,
                before,
                after,
            } => EvmStandardChange::OperatorApproval {
                contract_address: contract,
                owner,
                operator,
                before,
                after,
            },
            Self::Erc1155TransferSingle {
                contract,
                operator,
                from,
                to,
                token_id,
                amount,
            } => {
                if from == Address::ZERO {
                    EvmStandardChange::Erc1155MintSingle {
                        contract_address: contract,
                        operator,
                        to,
                        token_id,
                        raw_amount: amount,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc1155BurnSingle {
                        contract_address: contract,
                        operator,
                        from,
                        token_id,
                        raw_amount: amount,
                    }
                } else {
                    EvmStandardChange::Erc1155TransferSingle {
                        contract_address: contract,
                        operator,
                        from,
                        to,
                        token_id,
                        raw_amount: amount,
                    }
                }
            }
            Self::Erc1155TransferBatch {
                contract,
                operator,
                from,
                to,
                items,
            } => {
                let items = items
                    .into_iter()
                    .map(|(token_id, raw_amount)| Erc1155TransferItem {
                        token_id,
                        raw_amount,
                    })
                    .collect();
                if from == Address::ZERO {
                    EvmStandardChange::Erc1155MintBatch {
                        contract_address: contract,
                        operator,
                        to,
                        items,
                    }
                } else if to == Address::ZERO {
                    EvmStandardChange::Erc1155BurnBatch {
                        contract_address: contract,
                        operator,
                        from,
                        items,
                    }
                } else {
                    EvmStandardChange::Erc1155TransferBatch {
                        contract_address: contract,
                        operator,
                        from,
                        to,
                        items,
                    }
                }
            }
        }
    }
}
