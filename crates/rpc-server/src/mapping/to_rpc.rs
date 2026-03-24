use crate::interface as rpc;

use super::primitives::format_u64_quantity;

impl From<simulation_service::SimulateEvmTransactionOutput>
    for rpc::EvmSimulateTransactionResponse
{
    fn from(output: simulation_service::SimulateEvmTransactionOutput) -> Self {
        Self {
            chain_id: format_u64_quantity(output.chain_id),
            block: output.block.into(),
            status: output.status.into(),
            gas_used: format_u64_quantity(output.gas_used),
            gas_limit: format_u64_quantity(output.gas_limit),
            output: format!("{:#x}", output.output),
            failure: output.failure.map(Into::into),
            logs: output.logs.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<simulation_service::SimulatedBlock> for rpc::SimulatedBlock {
    fn from(block: simulation_service::SimulatedBlock) -> Self {
        Self {
            number: format_u64_quantity(block.number),
            hash: format!("{:#x}", block.hash),
        }
    }
}

impl From<simulation_service::SimulationStatus> for rpc::SimulationStatus {
    fn from(status: simulation_service::SimulationStatus) -> Self {
        match status {
            simulation_service::SimulationStatus::Success => Self::Success,
            simulation_service::SimulationStatus::Failed => Self::Failed,
        }
    }
}

impl From<simulation_service::SimulationFailure> for rpc::SimulationFailure {
    fn from(failure: simulation_service::SimulationFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<simulation_service::RawLog> for rpc::RawLog {
    fn from(log: simulation_service::RawLog) -> Self {
        Self {
            log_index: format_u64_quantity(log.log_index),
            address: format!("{:#x}", log.address),
            topics: log
                .topics
                .into_iter()
                .map(|topic| format!("{:#x}", topic))
                .collect(),
            data: format!("{:#x}", log.data),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes};

    use crate::interface as rpc;

    #[test]
    fn service_output_maps_into_rpc_response() {
        let output = simulation_service::SimulateEvmTransactionOutput {
            chain_id: 1,
            block: simulation_service::SimulatedBlock {
                number: 0x1234,
                hash: B256::from_str(
                    "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("hash"),
            },
            status: simulation_service::SimulationStatus::Success,
            gas_used: 0x5208,
            gas_limit: 0x5300,
            output: Bytes::from_str("0x0102").expect("bytes"),
            failure: None,
            logs: vec![simulation_service::RawLog {
                log_index: 0,
                address: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("address"),
                topics: vec![
                    B256::from_str(
                        "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    )
                    .expect("topic"),
                ],
                data: Bytes::from_str("0xdeadbeef").expect("bytes"),
            }],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.chain_id, "0x1");
        assert_eq!(response.block.number, "0x1234");
        assert_eq!(response.gas_used, "0x5208");
        assert_eq!(response.logs.len(), 1);
        assert_eq!(response.logs[0].log_index, "0x0");
        assert_eq!(response.output, "0x0102");
    }
}
