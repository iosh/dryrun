use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use alloy_json_rpc::{RpcRecv, RpcSend};
use alloy_rpc_client::{BatchRequest, Waiter};

use crate::{
    ConfluxProvider, ConfluxProviderError, CoreAccount, CoreAddress, CoreCollateralInfo,
    CorePoSEconomics, CoreSupplyInfo, CoreVoteParams, EpochNumber,
    error::{classify_alloy_rpc_error, classify_batch_error},
};
use alloy_primitives::U256;

#[must_use = "a batch call must be awaited after the batch is sent"]
pub struct BatchCall<T> {
    inner: Pin<Box<dyn Future<Output = Result<T, ConfluxProviderError>> + Send>>,
}

impl<T> Future for BatchCall<T> {
    type Output = Result<T, ConfluxProviderError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(context)
    }
}

pub struct CoreBatch<'a> {
    provider: &'a ConfluxProvider,
    inner: BatchRequest<'a>,
}

impl<'a> CoreBatch<'a> {
    pub(crate) fn new(provider: &'a ConfluxProvider) -> Self {
        Self {
            provider,
            inner: provider.client().new_batch(),
        }
    }

    pub fn cfx_get_interest_rate(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<U256>, ConfluxProviderError> {
        self.add("cfx_getInterestRate", (epoch,))
    }

    pub fn cfx_get_accumulate_interest_rate(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<U256>, ConfluxProviderError> {
        self.add("cfx_getAccumulateInterestRate", (epoch,))
    }

    pub fn cfx_get_supply_info(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<CoreSupplyInfo>, ConfluxProviderError> {
        self.add("cfx_getSupplyInfo", (epoch,))
    }

    pub fn cfx_get_collateral_info(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<CoreCollateralInfo>, ConfluxProviderError> {
        self.add("cfx_getCollateralInfo", (epoch,))
    }

    pub fn cfx_get_pos_economics(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<CorePoSEconomics>, ConfluxProviderError> {
        self.add("cfx_getPoSEconomics", (epoch,))
    }

    pub fn cfx_get_params_from_vote(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<CoreVoteParams>, ConfluxProviderError> {
        self.add("cfx_getParamsFromVote", (epoch,))
    }

    pub fn cfx_get_fee_burnt(
        &mut self,
        epoch: EpochNumber,
    ) -> Result<BatchCall<U256>, ConfluxProviderError> {
        self.add("cfx_getFeeBurnt", (epoch,))
    }

    pub fn cfx_get_account(
        &mut self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<BatchCall<CoreAccount>, ConfluxProviderError> {
        let waiter = self.add_waiter("cfx_getAccount", (address, epoch))?;
        let provider = self.provider.clone();
        Ok(self.decode(waiter, "cfx_getAccount", move |wire| {
            provider.decode_account("cfx_getAccount", wire)
        }))
    }

    pub fn cfx_get_collateral_for_storage(
        &mut self,
        address: CoreAddress,
        epoch: EpochNumber,
    ) -> Result<BatchCall<U256>, ConfluxProviderError> {
        self.add("cfx_getCollateralForStorage", (address, epoch))
    }

    pub async fn send(self) -> Result<(), ConfluxProviderError> {
        self.inner
            .send()
            .await
            .map_err(|error| classify_alloy_rpc_error("Core Space typed batch", error))
    }

    fn add<Params, Response>(
        &mut self,
        method: &'static str,
        params: Params,
    ) -> Result<BatchCall<Response>, ConfluxProviderError>
    where
        Params: RpcSend,
        Response: RpcRecv,
    {
        let waiter = self.add_waiter(method, params)?;
        Ok(self.decode(waiter, method, Ok))
    }

    fn add_waiter<Params, Response>(
        &mut self,
        method: &'static str,
        params: Params,
    ) -> Result<Waiter<Response>, ConfluxProviderError>
    where
        Params: RpcSend,
        Response: RpcRecv,
    {
        self.inner
            .add_call(method, &params)
            .map_err(|error| classify_alloy_rpc_error(method, error))
    }

    fn decode<Response, Output, Decode>(
        &self,
        waiter: Waiter<Response>,
        method: &'static str,
        decode: Decode,
    ) -> BatchCall<Output>
    where
        Response: RpcRecv,
        Output: Send + 'static,
        Decode: FnOnce(Response) -> Result<Output, ConfluxProviderError> + Send + 'static,
    {
        BatchCall {
            inner: Box::pin(async move {
                let response = waiter
                    .await
                    .map_err(|error| classify_batch_error(crate::CORE_BATCH_NAME, method, error))?;
                decode(response)
            }),
        }
    }
}
