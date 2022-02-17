// Copyright 2021 UINB Technologies Pte. Ltd.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub use self::gen_client::Client as FusoVerifierClient;
pub use fuso_verifier_runtime_api::FusoVerifierRuntimeApi;

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::sync::Arc;

#[rpc]
pub trait FusoVerifierApi<BlockHash, AccountId, Balance> {
    #[rpc(name = "verifier_currentSeasonOfDominator")]
    fn current_season_of_dominator(
        &self,
        dominator: AccountId,
        at: Option<BlockHash>,
    ) -> Result<u32>;

    #[rpc(name = "verifier_pendingSharesOfDominator")]
    fn pending_shares_of_dominator(
        &self,
        dominator: AccountId,
        who: AccountId,
        at: Option<BlockHash>,
    ) -> Result<NumberOrHex>;
}

pub struct FusoVerifier<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> FusoVerifier<C, B> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AccountId, Balance> FusoVerifierApi<<Block as BlockT>::Hash, AccountId, Balance>
    for FusoVerifier<C, (Block, AccountId, Balance)>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: FusoVerifierRuntimeApi<Block, AccountId, Balance>,
    AccountId: Codec + MaybeDisplay + Send + Sync + 'static,
    Balance: Codec + MaybeDisplay + TryInto<NumberOrHex> + Send + Sync + 'static,
{
    fn current_season_of_dominator(
        &self,
        dominator: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<u32> {
        let api = self.client.runtime_api();
        let block_hash = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.current_season_of_dominator(&block_hash, dominator)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(0),
                message: "Unable to query current season".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn pending_shares_of_dominator(
        &self,
        dominator: AccountId,
        who: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<NumberOrHex> {
        let api = self.client.runtime_api();
        let block_hash = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let shares = api
            .pending_shares_of_dominator(&block_hash, dominator, who)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(1),
                message: "Unable to query pending shares".into(),
                data: Some(format!("{:?}", e).into()),
            })?;
        shares.try_into().map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "doesn't fit in NumberOrHex representation".into(),
            data: None,
        })
    }
}
