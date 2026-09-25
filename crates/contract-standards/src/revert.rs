use std::fmt;

use alloy_primitives::U256;
use alloy_sol_types::{Panic, Revert, SolError};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SolidityRevertReason {
    SolidityError { message: String },
    SolidityPanic { code: U256 },
}

impl SolidityRevertReason {
    pub fn decode(output: &[u8]) -> Option<Self> {
        Revert::abi_decode_validate(output)
            .map(|revert| Self::SolidityError {
                message: revert.reason,
            })
            .or_else(|_| {
                Panic::abi_decode_validate(output)
                    .map(|panic| Self::SolidityPanic { code: panic.code })
            })
            .ok()
    }
}

impl fmt::Display for SolidityRevertReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SolidityError { message } if message.is_empty() => formatter.write_str("<empty>"),
            Self::SolidityError { message } => formatter.write_str(message),
            Self::SolidityPanic { code } => {
                formatter.write_str(Panic { code: *code }.as_geth_str().as_ref())
            }
        }
    }
}
