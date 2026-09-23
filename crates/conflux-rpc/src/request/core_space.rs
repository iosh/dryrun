use cfx_rpc_cfx_types::EpochNumber;
use conflux_simulation::core_space::{
    CoreSpaceBlockSelector, CoreSpaceSimulationRequest, CoreSpaceTransactionInput,
    CoreSpaceTransactionRequest,
};
use serde::Deserialize;

use crate::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SimulateCoreSpaceTransactionRequest {
    transaction: CoreSpaceTransactionRequest,
    #[serde(default)]
    epoch: Option<EpochNumber>,
}

impl TryFrom<SimulateCoreSpaceTransactionRequest> for CoreSpaceSimulationRequest {
    type Error = ValidationError;

    fn try_from(request: SimulateCoreSpaceTransactionRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            block: map_core_space_epoch(request.epoch)?,
            transaction: CoreSpaceTransactionInput::Partial(request.transaction),
        })
    }
}

fn map_core_space_epoch(
    epoch: Option<EpochNumber>,
) -> Result<CoreSpaceBlockSelector, ValidationError> {
    match epoch.unwrap_or(EpochNumber::LatestState) {
        EpochNumber::LatestState => Ok(CoreSpaceBlockSelector::LatestState),
        EpochNumber::Num(number) => Ok(CoreSpaceBlockSelector::Number(number.as_u64())),
        _ => Err(ValidationError::not_supported(
            "`epoch` only supports `latest_state` or a hex epoch number",
        )),
    }
}
