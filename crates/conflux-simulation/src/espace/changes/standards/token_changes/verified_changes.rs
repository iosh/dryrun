use alloy::primitives::{Address, U256};
use contract_standards::Erc1155TransferItem;

use crate::espace::changes::EspaceStandardChange;

#[derive(Debug)]
pub(super) enum VerifiedTokenChange {
    Standard(VerifiedStandardChange),
    Wrapped,
}

#[derive(Debug)]
pub(super) enum VerifiedStandardChange {
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

impl VerifiedStandardChange {
    pub(super) fn into_change(self) -> EspaceStandardChange {
        match self {
            Self::Erc20Transfer {
                contract,
                from,
                to,
                amount,
            } => {
                if from == Address::ZERO {
                    EspaceStandardChange::Erc20Mint {
                        contract_address: contract,
                        to,
                        raw_amount: amount,
                    }
                } else if to == Address::ZERO {
                    EspaceStandardChange::Erc20Burn {
                        contract_address: contract,
                        from,
                        raw_amount: amount,
                    }
                } else {
                    EspaceStandardChange::Erc20Transfer {
                        contract_address: contract,
                        from,
                        to,
                        raw_amount: amount,
                    }
                }
            }
            Self::Erc20Approval {
                contract,
                owner,
                spender,
                before,
                after,
            } => EspaceStandardChange::Erc20Approval {
                contract_address: contract,
                owner,
                spender,
                before,
                after,
            },
            Self::Erc721Transfer {
                contract,
                from,
                to,
                token_id,
            } => {
                if from == Address::ZERO {
                    EspaceStandardChange::Erc721Mint {
                        contract_address: contract,
                        to,
                        token_id,
                    }
                } else if to == Address::ZERO {
                    EspaceStandardChange::Erc721Burn {
                        contract_address: contract,
                        from,
                        token_id,
                    }
                } else {
                    EspaceStandardChange::Erc721Transfer {
                        contract_address: contract,
                        from,
                        to,
                        token_id,
                    }
                }
            }
            Self::Erc721Approval {
                contract,
                owner,
                before,
                after,
                token_id,
            } => EspaceStandardChange::Erc721Approval {
                contract_address: contract,
                owner,
                before,
                after,
                token_id,
            },
            Self::OperatorApproval {
                contract,
                owner,
                operator,
                before,
                after,
            } => EspaceStandardChange::OperatorApproval {
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
                    EspaceStandardChange::Erc1155MintSingle {
                        contract_address: contract,
                        operator,
                        to,
                        token_id,
                        raw_amount: amount,
                    }
                } else if to == Address::ZERO {
                    EspaceStandardChange::Erc1155BurnSingle {
                        contract_address: contract,
                        operator,
                        from,
                        token_id,
                        raw_amount: amount,
                    }
                } else {
                    EspaceStandardChange::Erc1155TransferSingle {
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
                    EspaceStandardChange::Erc1155MintBatch {
                        contract_address: contract,
                        operator,
                        to,
                        items,
                    }
                } else if to == Address::ZERO {
                    EspaceStandardChange::Erc1155BurnBatch {
                        contract_address: contract,
                        operator,
                        from,
                        items,
                    }
                } else {
                    EspaceStandardChange::Erc1155TransferBatch {
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
