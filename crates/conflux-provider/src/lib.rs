mod address;
mod batch;
mod cfx;
mod error;
mod types;

use alloy_rpc_client::RpcClient;

pub use address::{AddressError, CoreAddress, Network, NetworkError};
pub use batch::{BatchCall, CoreBatch};
pub use error::Error;
pub use types::*;

#[derive(Debug, Clone)]
pub struct ConfluxProvider {
    client: RpcClient,
}

impl ConfluxProvider {
    pub fn new(client: RpcClient) -> Self {
        Self { client }
    }

    pub fn batch(&self) -> CoreBatch<'_> {
        CoreBatch::new(self)
    }

    async fn request<Params, Response>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<Response, Error>
    where
        Params: alloy_json_rpc::RpcSend,
        Response: alloy_json_rpc::RpcRecv,
    {
        self.client
            .request(method, params)
            .await
            .map_err(|error| Error::Rpc {
                method,
                source: error,
            })
    }

    async fn request_noparams<Response>(&self, method: &'static str) -> Result<Response, Error>
    where
        Response: alloy_json_rpc::RpcRecv,
    {
        self.client
            .request_noparams::<Response>(method)
            .await
            .map_err(|error| Error::Rpc {
                method,
                source: error,
            })
    }
}
