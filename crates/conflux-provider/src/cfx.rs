use crate::{
    BalanceCheck, BalanceCheckRequest, BlockHashOrEpochNumber, ConfluxProvider,
    ConfluxProviderError, CoreAccount, CoreAddress, CoreCollateralInfo, CoreFeeHistory, CoreLog,
    CoreLogFilter, CorePendingInfo, CorePendingTransactions, CorePoSEconomics, CoreReceipt,
    CoreRewardInfo, CoreRpcBlock, CoreRpcTransaction, CoreStatus, CoreStorageRoot, CoreSupplyInfo,
    CoreTransactionRequest, CoreVoteParams, DepositInfo, EpochNumber,
    EstimateGasAndCollateralRequest, GasAndCollateralEstimate, PosEpochReward, VoteStakeInfo,
};
use alloy_primitives::{B256, Bytes, U256};

impl ConfluxProvider {
    pub async fn cfx_get_next_nonce(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getNextNonce", (address, selector)).await
    }

    pub async fn cfx_gas_price(&self) -> Result<U256, ConfluxProviderError> {
        self.request_noparams("cfx_gasPrice").await
    }

    pub async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, ConfluxProviderError> {
        self.request_noparams("cfx_maxPriorityFeePerGas").await
    }

    pub async fn cfx_estimate_gas_and_collateral(
        &self,
        request: EstimateGasAndCollateralRequest,
        epoch: EpochNumber,
    ) -> Result<GasAndCollateralEstimate, ConfluxProviderError> {
        self.request("cfx_estimateGasAndCollateral", (request, epoch))
            .await
    }

    pub async fn cfx_check_balance_against_transaction(
        &self,
        request: BalanceCheckRequest,
        epoch: EpochNumber,
    ) -> Result<BalanceCheck, ConfluxProviderError> {
        self.request(
            "cfx_checkBalanceAgainstTransaction",
            (
                request.account,
                request.contract,
                request.gas_limit,
                request.gas_price,
                request.storage_limit,
                Some(epoch),
            ),
        )
        .await
    }

    pub async fn cfx_get_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getInterestRate", (epoch,)).await
    }

    pub async fn cfx_get_accumulate_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getAccumulateInterestRate", (epoch,))
            .await
    }

    pub async fn cfx_get_supply_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreSupplyInfo, ConfluxProviderError> {
        self.request("cfx_getSupplyInfo", (epoch,)).await
    }

    pub async fn cfx_get_collateral_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreCollateralInfo, ConfluxProviderError> {
        self.request("cfx_getCollateralInfo", (epoch,)).await
    }

    pub async fn cfx_get_pos_economics(
        &self,
        epoch: EpochNumber,
    ) -> Result<CorePoSEconomics, ConfluxProviderError> {
        self.request("cfx_getPoSEconomics", (epoch,)).await
    }

    pub async fn cfx_get_params_from_vote(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreVoteParams, ConfluxProviderError> {
        self.request("cfx_getParamsFromVote", (epoch,)).await
    }

    pub async fn cfx_get_fee_burnt(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getFeeBurnt", (epoch,)).await
    }

    pub async fn cfx_get_account(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<CoreAccount, ConfluxProviderError> {
        let wire: super::CoreAccountWire = self.request("cfx_getAccount", (address, epoch)).await?;
        self.decode_account("cfx_getAccount", wire)
    }

    pub async fn cfx_get_collateral_for_storage(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<U256, ConfluxProviderError> {
        self.request("cfx_getCollateralForStorage", (address, epoch))
            .await
    }

    pub async fn cfx_get_deposit_list(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<Vec<DepositInfo>, ConfluxProviderError> {
        self.request("cfx_getDepositList", (address, epoch)).await
    }

    pub async fn cfx_get_vote_list(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<Vec<VoteStakeInfo>, ConfluxProviderError> {
        self.request("cfx_getVoteList", (address, epoch)).await
    }

    pub async fn cfx_get_sponsor_info(
        &self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<crate::CoreSponsorInfo, ConfluxProviderError> {
        let wire: super::CoreSponsorInfoWire =
            self.request("cfx_getSponsorInfo", (address, epoch)).await?;
        self.decode_sponsor_info("cfx_getSponsorInfo", wire)
    }

    pub async fn cfx_get_code(
        &self,
        address: CoreAddress,
        selector: BlockHashOrEpochNumber,
    ) -> Result<Bytes, ConfluxProviderError> {
        self.request("cfx_getCode", (address, selector)).await
    }

    pub async fn cfx_epoch_number(
        &self,
        selector: Option<EpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        match selector {
            Some(selector) => self.request("cfx_epochNumber", (selector,)).await,
            None => self.request_noparams("cfx_epochNumber").await,
        }
    }

    pub async fn cfx_get_balance(
        &self,
        address: CoreAddress,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        match selector {
            Some(selector) => self.request("cfx_getBalance", (address, selector)).await,
            None => self.request("cfx_getBalance", (address,)).await,
        }
    }

    pub async fn cfx_get_admin(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<Option<CoreAddress>, ConfluxProviderError> {
        let result: Option<CoreAddress> = match epoch {
            Some(epoch) => self.request("cfx_getAdmin", (address, epoch)).await?,
            None => self.request("cfx_getAdmin", (address,)).await?,
        };
        Ok(result)
    }

    pub async fn cfx_get_staking_balance(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        match epoch {
            Some(epoch) => {
                self.request("cfx_getStakingBalance", (address, epoch))
                    .await
            }
            None => self.request("cfx_getStakingBalance", (address,)).await,
        }
    }

    pub async fn cfx_get_storage_root(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<Option<CoreStorageRoot>, ConfluxProviderError> {
        match epoch {
            Some(epoch) => self.request("cfx_getStorageRoot", (address, epoch)).await,
            None => self.request("cfx_getStorageRoot", (address,)).await,
        }
    }

    pub async fn cfx_get_block_by_hash(
        &self,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<CoreRpcBlock>, ConfluxProviderError> {
        let block: Option<CoreRpcBlock> = self
            .request("cfx_getBlockByHash", (hash, include_transactions))
            .await?;
        Ok(block)
    }

    pub async fn cfx_get_block_by_hash_with_pivot_assumption(
        &self,
        hash: B256,
        pivot_hash: B256,
        epoch: U256,
    ) -> Result<CoreRpcBlock, ConfluxProviderError> {
        let block: CoreRpcBlock = self
            .request(
                "cfx_getBlockByHashWithPivotAssumption",
                (hash, pivot_hash, epoch),
            )
            .await?;
        Ok(block)
    }

    pub async fn cfx_get_block_by_epoch_number(
        &self,
        epoch: EpochNumber,
        include_transactions: bool,
    ) -> Result<Option<CoreRpcBlock>, ConfluxProviderError> {
        let block: Option<CoreRpcBlock> = self
            .request("cfx_getBlockByEpochNumber", (epoch, include_transactions))
            .await?;
        Ok(block)
    }

    pub async fn cfx_get_block_by_block_number(
        &self,
        number: U256,
        include_transactions: bool,
    ) -> Result<Option<CoreRpcBlock>, ConfluxProviderError> {
        let block: Option<CoreRpcBlock> = self
            .request("cfx_getBlockByBlockNumber", (number, include_transactions))
            .await?;
        Ok(block)
    }

    pub async fn cfx_get_best_block_hash(&self) -> Result<B256, ConfluxProviderError> {
        self.request_noparams("cfx_getBestBlockHash").await
    }

    pub async fn cfx_get_next_nonce_at(
        &self,
        address: CoreAddress,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        match selector {
            Some(selector) => self.request("cfx_getNextNonce", (address, selector)).await,
            None => self.request("cfx_getNextNonce", (address,)).await,
        }
    }

    pub async fn cfx_get_code_at(
        &self,
        address: CoreAddress,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<Bytes, ConfluxProviderError> {
        match selector {
            Some(selector) => self.request("cfx_getCode", (address, selector)).await,
            None => self.request("cfx_getCode", (address,)).await,
        }
    }

    pub async fn cfx_get_storage_at(
        &self,
        address: CoreAddress,
        slot: U256,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<Option<B256>, ConfluxProviderError> {
        match selector {
            Some(selector) => {
                self.request("cfx_getStorageAt", (address, slot, selector))
                    .await
            }
            None => self.request("cfx_getStorageAt", (address, slot)).await,
        }
    }

    pub async fn cfx_get_sponsor_info_at(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::CoreSponsorInfo, ConfluxProviderError> {
        let wire: super::CoreSponsorInfoWire = match epoch {
            Some(epoch) => self.request("cfx_getSponsorInfo", (address, epoch)).await?,
            None => self.request("cfx_getSponsorInfo", (address,)).await?,
        };
        self.decode_sponsor_info("cfx_getSponsorInfo", wire)
    }

    pub async fn cfx_get_deposit_list_at(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<Vec<crate::DepositInfo>, ConfluxProviderError> {
        match epoch {
            Some(epoch) => self.request("cfx_getDepositList", (address, epoch)).await,
            None => self.request("cfx_getDepositList", (address,)).await,
        }
    }

    pub async fn cfx_get_vote_list_at(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<Vec<crate::VoteStakeInfo>, ConfluxProviderError> {
        match epoch {
            Some(epoch) => self.request("cfx_getVoteList", (address, epoch)).await,
            None => self.request("cfx_getVoteList", (address,)).await,
        }
    }

    pub async fn cfx_get_collateral_for_storage_at(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        match epoch {
            Some(epoch) => {
                self.request("cfx_getCollateralForStorage", (address, epoch))
                    .await
            }
            None => {
                self.request("cfx_getCollateralForStorage", (address,))
                    .await
            }
        }
    }

    pub async fn cfx_send_raw_transaction(&self, raw: Bytes) -> Result<B256, ConfluxProviderError> {
        self.request("cfx_sendRawTransaction", (raw,)).await
    }

    pub async fn cfx_call(
        &self,
        request: CoreTransactionRequest,
        selector: Option<crate::BlockHashOrEpochNumber>,
    ) -> Result<Bytes, ConfluxProviderError> {
        match selector {
            Some(selector) => self.request("cfx_call", (request, selector)).await,
            None => self.request("cfx_call", (request,)).await,
        }
    }

    pub async fn cfx_get_logs(
        &self,
        filter: CoreLogFilter,
    ) -> Result<Vec<CoreLog>, ConfluxProviderError> {
        self.request("cfx_getLogs", (filter,)).await
    }

    pub async fn cfx_get_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<CoreRpcTransaction>, ConfluxProviderError> {
        let transaction: Option<CoreRpcTransaction> =
            self.request("cfx_getTransactionByHash", (hash,)).await?;
        Ok(transaction)
    }

    pub async fn cfx_get_account_pending_info(
        &self,
        address: CoreAddress,
    ) -> Result<Option<CorePendingInfo>, ConfluxProviderError> {
        self.request("cfx_getAccountPendingInfo", (address,)).await
    }

    pub async fn cfx_get_account_pending_transactions(
        &self,
        address: CoreAddress,
        start_nonce: Option<U256>,
        limit: Option<U256>,
    ) -> Result<CorePendingTransactions, ConfluxProviderError> {
        let result: CorePendingTransactions = self
            .request(
                "cfx_getAccountPendingTransactions",
                (address, start_nonce, limit),
            )
            .await?;
        Ok(result)
    }

    pub async fn cfx_estimate_gas_and_collateral_at(
        &self,
        request: crate::EstimateGasAndCollateralRequest,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::GasAndCollateralEstimate, ConfluxProviderError> {
        match epoch {
            Some(epoch) => {
                self.request("cfx_estimateGasAndCollateral", (request, epoch))
                    .await
            }
            None => {
                self.request("cfx_estimateGasAndCollateral", (request,))
                    .await
            }
        }
    }

    pub async fn cfx_fee_history(
        &self,
        block_count: U256,
        newest_epoch: EpochNumber,
        reward_percentiles: Option<Vec<f64>>,
    ) -> Result<CoreFeeHistory, ConfluxProviderError> {
        self.request(
            "cfx_feeHistory",
            (block_count, newest_epoch, reward_percentiles),
        )
        .await
    }

    pub async fn cfx_check_balance_against_transaction_at(
        &self,
        account: CoreAddress,
        contract: CoreAddress,
        gas_limit: U256,
        gas_price: U256,
        storage_limit: U256,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::BalanceCheck, ConfluxProviderError> {
        match epoch {
            Some(epoch) => {
                self.request(
                    "cfx_checkBalanceAgainstTransaction",
                    (
                        account,
                        contract,
                        gas_limit,
                        gas_price,
                        storage_limit,
                        epoch,
                    ),
                )
                .await
            }
            None => {
                self.request(
                    "cfx_checkBalanceAgainstTransaction",
                    (account, contract, gas_limit, gas_price, storage_limit),
                )
                .await
            }
        }
    }

    pub async fn cfx_get_blocks_by_epoch(
        &self,
        epoch: EpochNumber,
    ) -> Result<Vec<B256>, ConfluxProviderError> {
        self.request("cfx_getBlocksByEpoch", (epoch,)).await
    }

    pub async fn cfx_get_skipped_blocks_by_epoch(
        &self,
        epoch: EpochNumber,
    ) -> Result<Vec<B256>, ConfluxProviderError> {
        self.request("cfx_getSkippedBlocksByEpoch", (epoch,)).await
    }

    pub async fn cfx_get_transaction_receipt(
        &self,
        hash: B256,
    ) -> Result<Option<CoreReceipt>, ConfluxProviderError> {
        let receipt: Option<CoreReceipt> =
            self.request("cfx_getTransactionReceipt", (hash,)).await?;
        Ok(receipt)
    }

    pub async fn cfx_get_account_at(
        &self,
        address: CoreAddress,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::CoreAccount, ConfluxProviderError> {
        let wire: super::CoreAccountWire = match epoch {
            Some(epoch) => self.request("cfx_getAccount", (address, epoch)).await?,
            None => self.request("cfx_getAccount", (address,)).await?,
        };
        self.decode_account("cfx_getAccount", wire)
    }

    pub async fn cfx_get_interest_rate_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getInterestRate", epoch).await
    }

    pub async fn cfx_get_accumulate_interest_rate_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getAccumulateInterestRate", epoch).await
    }

    pub async fn cfx_get_pos_economics_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::CorePoSEconomics, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getPoSEconomics", epoch).await
    }

    pub async fn cfx_get_supply_info_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::CoreSupplyInfo, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getSupplyInfo", epoch).await
    }

    pub async fn cfx_get_collateral_info_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::CoreCollateralInfo, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getCollateralInfo", epoch).await
    }

    pub async fn cfx_get_fee_burnt_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<U256, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getFeeBurnt", epoch).await
    }

    pub async fn cfx_get_params_from_vote_at(
        &self,
        epoch: Option<EpochNumber>,
    ) -> Result<crate::CoreVoteParams, ConfluxProviderError> {
        optional_epoch_call(self, "cfx_getParamsFromVote", epoch).await
    }

    pub async fn cfx_get_confirmation_risk_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<U256>, ConfluxProviderError> {
        self.request("cfx_getConfirmationRiskByHash", (hash,)).await
    }

    pub async fn cfx_get_block_reward_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<Vec<CoreRewardInfo>, ConfluxProviderError> {
        self.request("cfx_getBlockRewardInfo", (epoch,)).await
    }

    pub async fn cfx_get_pos_reward_by_epoch(
        &self,
        epoch: EpochNumber,
    ) -> Result<Option<PosEpochReward>, ConfluxProviderError> {
        self.request("cfx_getPoSRewardByEpoch", (epoch,)).await
    }

    pub async fn cfx_get_epoch_receipts(
        &self,
        selector: crate::BlockHashOrEpochNumber,
        include_eth_receipts: Option<bool>,
    ) -> Result<Option<Vec<Vec<CoreReceipt>>>, ConfluxProviderError> {
        let result: Option<Vec<Vec<CoreReceipt>>> = match include_eth_receipts {
            Some(include_eth_receipts) => {
                self.request("cfx_getEpochReceipts", (selector, include_eth_receipts))
                    .await?
            }
            None => self.request("cfx_getEpochReceipts", (selector,)).await?,
        };
        Ok(result)
    }

    pub async fn cfx_get_status(&self) -> Result<CoreStatus, ConfluxProviderError> {
        self.request_noparams("cfx_getStatus").await
    }

    pub async fn cfx_client_version(&self) -> Result<String, ConfluxProviderError> {
        self.request_noparams("cfx_clientVersion").await
    }
}

async fn optional_epoch_call<T>(
    provider: &ConfluxProvider,
    method: &'static str,
    epoch: Option<EpochNumber>,
) -> Result<T, ConfluxProviderError>
where
    T: alloy_json_rpc::RpcRecv,
{
    match epoch {
        Some(epoch) => provider.request(method, (epoch,)).await,
        None => provider.request_noparams(method).await,
    }
}
