use crate::{ConfluxProvider, CoreAddress, Error, types::*};
use alloy_primitives::{B256, Bytes, U256};
use alloy_transport::TransportError;
use cfx_rpc_cfx_types::TransactionRequest;
use serde::Deserialize;

impl ConfluxProvider {
    pub async fn pos_get_block_by_hash(&self, hash: B256) -> Result<Option<PosBlock>, Error> {
        self.request("pos_getBlockByHash", (hash,)).await
    }

    pub async fn cfx_get_next_nonce(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<U256, Error> {
        self.request("cfx_getNextNonce", (address, selector)).await
    }

    pub async fn cfx_gas_price(&self) -> Result<U256, Error> {
        self.request_noparams("cfx_gasPrice").await
    }

    pub async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, Error> {
        self.request_noparams("cfx_maxPriorityFeePerGas").await
    }

    pub async fn cfx_estimate_gas_and_collateral(
        &self,
        request: TransactionRequest,
        epoch: EpochNumber,
    ) -> Result<GasAndCollateralEstimate, Error> {
        self.request("cfx_estimateGasAndCollateral", (request, epoch))
            .await
    }

    pub async fn cfx_get_interest_rate(&self, epoch: EpochNumber) -> Result<U256, Error> {
        self.request("cfx_getInterestRate", (epoch,)).await
    }

    pub async fn cfx_get_accumulate_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, Error> {
        self.request("cfx_getAccumulateInterestRate", (epoch,))
            .await
    }

    pub async fn cfx_get_supply_info(&self, epoch: EpochNumber) -> Result<CoreSupplyInfo, Error> {
        self.request("cfx_getSupplyInfo", (epoch,)).await
    }

    pub async fn cfx_get_collateral_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreCollateralInfo, Error> {
        self.request("cfx_getCollateralInfo", (epoch,)).await
    }

    pub async fn cfx_get_pos_economics(
        &self,
        epoch: EpochNumber,
    ) -> Result<CorePoSEconomics, Error> {
        self.request("cfx_getPoSEconomics", (epoch,)).await
    }

    pub async fn cfx_get_params_from_vote(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreVoteParams, Error> {
        self.request("cfx_getParamsFromVote", (epoch,)).await
    }

    pub async fn cfx_get_fee_burnt(&self, epoch: EpochNumber) -> Result<U256, Error> {
        self.request("cfx_getFeeBurnt", (epoch,)).await
    }

    pub async fn cfx_get_account(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<CoreAccount, Error> {
        self.request("cfx_getAccount", (address, epoch)).await
    }

    pub async fn cfx_get_collateral_for_storage(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<U256, Error> {
        self.request("cfx_getCollateralForStorage", (address, epoch))
            .await
    }

    pub async fn cfx_get_deposit_list(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<Vec<DepositInfo>, Error> {
        self.request("cfx_getDepositList", (address, epoch)).await
    }

    pub async fn cfx_get_vote_list(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<Vec<VoteStakeInfo>, Error> {
        self.request("cfx_getVoteList", (address, epoch)).await
    }

    pub async fn cfx_get_sponsor_info(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<crate::CoreSponsorInfo, Error> {
        self.request("cfx_getSponsorInfo", (address, epoch)).await
    }

    pub async fn cfx_get_code(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<Bytes, Error> {
        self.request("cfx_getCode", (address, selector)).await
    }

    pub async fn cfx_epoch_number(&self, selector: Option<EpochNumber>) -> Result<U256, Error> {
        match selector {
            Some(selector) => self.request("cfx_epochNumber", (selector,)).await,
            None => self.request_noparams("cfx_epochNumber").await,
        }
    }

    pub async fn cfx_get_block_by_hash(
        &self,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<CoreRpcBlock>, Error> {
        let block: Option<CoreRpcBlock> = self
            .request("cfx_getBlockByHash", (hash, include_transactions))
            .await?;
        Ok(block)
    }

    pub async fn cfx_get_block_by_epoch_number(
        &self,
        epoch: EpochNumber,
        include_transactions: bool,
    ) -> Result<Option<CoreRpcBlock>, Error> {
        let block: Option<CoreRpcBlock> = self
            .request("cfx_getBlockByEpochNumber", (epoch, include_transactions))
            .await?;
        Ok(block)
    }

    pub async fn cfx_get_storage_at(
        &self,
        address: CoreAddress,
        slot: U256,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<Option<B256>, Error> {
        let method = "cfx_getStorageAt";
        let response: serde_json::Value = match selector {
            Some(selector) => self.request(method, (address, slot, selector)).await?,
            None => self.request(method, (address, slot)).await?,
        };
        // Public endpoints may encode an absent slot as empty bytes instead of null.
        if response.as_str() == Some("0x") {
            return Ok(None);
        }
        Option::<B256>::deserialize(&response).map_err(|error| Error::Rpc {
            method,
            source: TransportError::deser_err(error, response.to_string()),
        })
    }

    pub async fn cfx_call(
        &self,
        request: TransactionRequest,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<Bytes, Error> {
        match selector {
            Some(selector) => self.request("cfx_call", (request, selector)).await,
            None => self.request("cfx_call", (request,)).await,
        }
    }

    pub async fn cfx_get_status(&self) -> Result<CoreStatus, Error> {
        self.request_noparams("cfx_getStatus").await
    }
}
