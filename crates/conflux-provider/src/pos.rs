use alloy_primitives::{B256, U256};

use crate::{
    ConfluxProvider, ConfluxProviderError, CoreAddress, PosAccount, PosBlockFull, PosBlockNumber,
    PosCommitteeState, PosEpochReward, PosLedgerInfoWithSignatures, PosStatus, PosTransaction,
};

impl ConfluxProvider {
    pub async fn pos_get_status(&self) -> Result<PosStatus, ConfluxProviderError> {
        self.request_noparams("pos_getStatus").await
    }

    pub async fn pos_get_account(
        &self,
        address: B256,
        view: Option<U256>,
    ) -> Result<PosAccount, ConfluxProviderError> {
        match view {
            Some(view) => self.request("pos_getAccount", (address, view)).await,
            None => self.request("pos_getAccount", (address,)).await,
        }
    }

    pub async fn pos_get_account_by_pow_address(
        &self,
        address: CoreAddress,
        view: Option<U256>,
    ) -> Result<PosAccount, ConfluxProviderError> {
        match view {
            Some(view) => {
                self.request("pos_getAccountByPowAddress", (address, view))
                    .await
            }
            None => self.request("pos_getAccountByPowAddress", (address,)).await,
        }
    }

    pub async fn pos_get_committee(
        &self,
        view: Option<U256>,
    ) -> Result<PosCommitteeState, ConfluxProviderError> {
        match view {
            Some(view) => self.request("pos_getCommittee", (view,)).await,
            None => self.request_noparams("pos_getCommittee").await,
        }
    }

    pub async fn pos_get_block_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<PosBlockFull>, ConfluxProviderError> {
        self.request("pos_getBlockByHash", (hash,)).await
    }

    pub async fn pos_get_block_by_number(
        &self,
        number: PosBlockNumber,
    ) -> Result<Option<PosBlockFull>, ConfluxProviderError> {
        self.request("pos_getBlockByNumber", (number,)).await
    }

    pub async fn pos_get_transaction_by_number(
        &self,
        number: U256,
    ) -> Result<Option<PosTransaction>, ConfluxProviderError> {
        self.request("pos_getTransactionByNumber", (number,)).await
    }

    pub async fn pos_get_ledger_info_by_block_number(
        &self,
        number: PosBlockNumber,
    ) -> Result<Option<PosLedgerInfoWithSignatures>, ConfluxProviderError> {
        self.request("pos_getLedgerInfoByBlockNumber", (number,))
            .await
    }

    pub async fn pos_get_ledger_info_by_epoch_and_round(
        &self,
        epoch: U256,
        round: U256,
    ) -> Result<Option<PosLedgerInfoWithSignatures>, ConfluxProviderError> {
        self.request("pos_getLedgerInfoByEpochAndRound", (epoch, round))
            .await
    }

    pub async fn pos_get_rewards_by_epoch(
        &self,
        epoch: U256,
    ) -> Result<Option<PosEpochReward>, ConfluxProviderError> {
        self.request("pos_getRewardsByEpoch", (epoch,)).await
    }
}
