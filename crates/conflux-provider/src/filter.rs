use crate::{
    ConfluxProvider, ConfluxProviderError, CoreFilterChanges, CoreFilterId, CoreFilterLog,
    CoreLogFilter,
};

impl ConfluxProvider {
    pub async fn cfx_new_filter(
        &self,
        filter: CoreLogFilter,
    ) -> Result<CoreFilterId, ConfluxProviderError> {
        let id = self.request("cfx_newFilter", (filter,)).await?;
        Ok(CoreFilterId(id))
    }

    pub async fn cfx_new_block_filter(&self) -> Result<CoreFilterId, ConfluxProviderError> {
        Ok(CoreFilterId(
            self.request_noparams("cfx_newBlockFilter").await?,
        ))
    }

    pub async fn cfx_new_pending_transaction_filter(
        &self,
    ) -> Result<CoreFilterId, ConfluxProviderError> {
        Ok(CoreFilterId(
            self.request_noparams("cfx_newPendingTransactionFilter")
                .await?,
        ))
    }

    pub async fn cfx_get_filter_changes(
        &self,
        filter: CoreFilterId,
    ) -> Result<CoreFilterChanges, ConfluxProviderError> {
        self.request("cfx_getFilterChanges", (filter.0,)).await
    }

    pub async fn cfx_get_filter_logs(
        &self,
        filter: CoreFilterId,
    ) -> Result<Vec<CoreFilterLog>, ConfluxProviderError> {
        self.request("cfx_getFilterLogs", (filter.0,)).await
    }

    pub async fn cfx_uninstall_filter(
        &self,
        filter: CoreFilterId,
    ) -> Result<bool, ConfluxProviderError> {
        self.request("cfx_uninstallFilter", (filter.0,)).await
    }
}
