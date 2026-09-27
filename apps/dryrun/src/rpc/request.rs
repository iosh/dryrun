use alloy::primitives::B256;
use conflux_simulation::{
    core_space::{
        CoreSpaceBlockSelector, CoreSpaceSimulationRequest, CoreSpaceTransactionInput,
        CoreSpaceTransactionRequest,
    },
    espace::{EspaceBlockSelector, EspaceSimulationRequest, EspaceTransactionInput},
};
use evm_simulation::{EvmBlockSelector, EvmSimulationRequest};
use serde::Deserialize;
use serde_json::Value;
use simulation_core::{
    error::{Diagnostic, ErrorCode},
    transaction::{TransactionInput, TransactionRequest},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockRequest {
    transaction: TransactionRequest,
    #[serde(default)]
    block: Option<BlockRef>,
    #[serde(default)]
    options: Option<ReservedOptions>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CoreRequest {
    transaction: CoreSpaceTransactionRequest,
    #[serde(default)]
    epoch: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BlockRef {
    Tag(String),
    Hash(HashRef),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HashRef {
    block_hash: B256,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReservedOptions {
    #[serde(default)]
    state_overrides: Option<Value>,
    #[serde(default)]
    block_overrides: Option<Value>,
    #[serde(default)]
    include: Option<Value>,
}

impl ReservedOptions {
    fn check_supported(options: Option<Self>) -> Result<(), Diagnostic> {
        if let Some(options) = options {
            for (field, value) in [
                ("stateOverrides", options.state_overrides),
                ("blockOverrides", options.block_overrides),
                ("include", options.include),
            ] {
                if value.is_some() {
                    return Err(unsupported(format!("options.{field} is not supported")));
                }
            }
        }
        Ok(())
    }
}

impl BlockRequest {
    pub(super) fn into_evm(self) -> Result<EvmSimulationRequest, Diagnostic> {
        ReservedOptions::check_supported(self.options)?;
        let block = match self.block {
            None => EvmBlockSelector::Latest,
            Some(BlockRef::Hash(hash)) => EvmBlockSelector::Hash(hash.block_hash),
            Some(BlockRef::Tag(tag)) => match tag.as_str() {
                "latest" => EvmBlockSelector::Latest,
                "safe" => EvmBlockSelector::Safe,
                "finalized" => EvmBlockSelector::Finalized,
                "pending" | "earliest" => {
                    return Err(unsupported("block selector is not supported"));
                }
                value => EvmBlockSelector::Number(parse_selector_number(value)?),
            },
        };
        Ok(EvmSimulationRequest {
            block,
            transaction: TransactionInput::Partial(self.transaction),
        })
    }

    pub(super) fn into_espace(self) -> Result<EspaceSimulationRequest, Diagnostic> {
        ReservedOptions::check_supported(self.options)?;
        let block = match self.block {
            None => EspaceBlockSelector::Latest,
            Some(BlockRef::Hash(hash)) => EspaceBlockSelector::Hash(hash.block_hash),
            Some(BlockRef::Tag(tag)) => match tag.as_str() {
                "latest" => EspaceBlockSelector::Latest,
                "pending" | "earliest" | "safe" | "finalized" => {
                    return Err(unsupported("block selector is not supported"));
                }
                value => EspaceBlockSelector::Number(parse_selector_number(value)?),
            },
        };
        Ok(EspaceSimulationRequest {
            block,
            transaction: EspaceTransactionInput::Partial(self.transaction),
        })
    }
}

impl CoreRequest {
    pub(super) fn into_simulation(self) -> Result<CoreSpaceSimulationRequest, Diagnostic> {
        let block = match self.epoch.as_deref() {
            None | Some("latest_state") => CoreSpaceBlockSelector::LatestState,
            Some(value) if value.starts_with("0x") => {
                CoreSpaceBlockSelector::Number(parse_selector_number(value)?)
            }
            Some(_) => {
                return Err(unsupported(
                    "epoch supports latest_state or a hexadecimal epoch number",
                ));
            }
        };
        Ok(CoreSpaceSimulationRequest {
            block,
            transaction: CoreSpaceTransactionInput::Partial(self.transaction),
        })
    }
}

fn parse_selector_number(value: &str) -> Result<u64, Diagnostic> {
    let digits = value
        .strip_prefix("0x")
        .filter(|digits| {
            !digits.is_empty()
                && !(digits.len() > 1 && digits.starts_with('0'))
                && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .ok_or_else(|| {
            Diagnostic::new(
                ErrorCode::InvalidInput,
                "selector requires a 0x-prefixed hexadecimal number without leading zeroes",
            )
        })?;
    u64::from_str_radix(digits, 16)
        .map_err(|error| Diagnostic::new(ErrorCode::InvalidInput, error.to_string()))
}

fn unsupported(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(ErrorCode::UnsupportedSimulation, message)
}
