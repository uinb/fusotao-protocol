// Copyright 2021-2023 UINB Technologies Pte. Ltd.

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

use codec::{Codec, Compact, Decode, Encode};
use jsonrpsee::{
    core::{error::Error as RpcError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
    types::SubscriptionResult,
    ws_server::SubscriptionSink,
};
use sc_service::SpawnTaskHandle;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::{
    crypto::{AccountId32, CryptoTypeId, CryptoTypePublicPair, KeyTypeId},
    Bytes, H256,
};
use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::sync::Arc;

// sha256
type Signature = H256;
type AccountId = AccountId32;

pub const RELAYER_KEY_TYPE: KeyTypeId = KeyTypeId(*b"rely");

#[derive(Eq, PartialEq, Clone, TypeInfo, Encode, Decode, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TradingCommand {
    Ask {
        order_id: String,
        account_id: AccountId,
        base: u32,
        quote: u32,
        amount: Compact<u128>,
        price: Compact<u128>,
        signature: Signature,
    },
    Bid {
        order_id: String,
        account_id: AccountId,
        base: u32,
        quote: u32,
        amount: Compact<u128>,
        price: Compact<u128>,
        signature: Signature,
    },
    Cancel {
        order_id: String,
        account_id: AccountId,
        signature: Signature,
    },
}

#[derive(Eq, PartialEq, Clone, Encode, Decode, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderEvent {
    order_id: String,
    account_id: AccountId,
    base: u32,
    quote: u32,
    state: u8,
    filled: Compact<u128>,
    price: Compact<u128>,
    update_at: u64,
}

#[derive(Eq, PartialEq, Clone, Encode, Decode, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderState {
    order_id: String,
    direction: u8,
    base: u32,
    quote: u32,
    state: u8,
    unfilled: Compact<u128>,
    total: Compact<u128>,
    price: Compact<u128>,
    filled_quote: Compact<u128>,
    update_at: u64,
}

/// relayer + runtime_api
#[rpc(client, server)]
pub trait FusoBrokerApi {
    #[method(name = "broker_placeOrder")]
    fn trade(&self, cmd: TradingCommand) -> RpcResult<String>;

    #[method(name = "broker_queryOrders")]
    fn query_orders(
        &self,
        account_id: AccountId,
        orders: Vec<(u32, u32, String)>,
        signature: Signature,
    ) -> RpcResult<Vec<Bytes>>;

    #[subscription(
        name = "broker_subscribeOrderEvents",
        unsubscribe = "broker_unsubscribeOrderEvents",
        item = Bytes,
    )]
    fn subscribe_order_events(&self, account_id: AccountId, signature: Signature);
}

pub struct FusoBroker<C, B> {
    client: Arc<C>,
    task_handle: SpawnTaskHandle,
    keystore: SyncCryptoStorePtr,
    _marker: std::marker::PhantomData<B>,
    // TODO keystore and executor
}

impl<C, B> FusoBroker<C, B> {
    pub fn new(client: Arc<C>, task_handle: SpawnTaskHandle, keystore: SyncCryptoStorePtr) -> Self {
        task_handle.spawn("broker-relayer", "fusotao", async {
            println!("===> TODO connect to prover");
        });
        Self {
            client,
            task_handle,
            keystore,
            _marker: Default::default(),
        }
    }
}

use sp_application_crypto::sr25519::CRYPTO_ID as Sr25519Id;

impl<C, B> FusoBroker<C, B> {
    /// the keystore is very unconvenient to use, be careful
    fn sign_request(&self, payload: &[u8]) -> Result<Vec<u8>, sp_keystore::Error> {
        let key = SyncCryptoStore::sr25519_public_keys(&*self.keystore, RELAYER_KEY_TYPE)
            .iter()
            .map(|k| CryptoTypePublicPair(Sr25519Id, k.0.to_vec()))
            .last()
            .ok_or(sp_keystore::Error::Unavailable)?;
        SyncCryptoStore::sign_with(&*self.keystore, RELAYER_KEY_TYPE, &key, payload)
            .transpose()
            .ok_or(sp_keystore::Error::Unavailable)?
    }
}

impl<C, Block> FusoBrokerApiServer for FusoBroker<C, Block>
where
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    Block: BlockT,
{
    fn trade(&self, cmd: TradingCommand) -> RpcResult<String> {
        let payload = cmd.encode();
        let v = self.sign_request(&payload).map_err(|e| {
            RpcError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::ServerError(93101i32).code(),
                "The broker hasn't register its signing key, please switch to another node.",
                Some(format!("{:?}", e)),
            )))
        })?;
        // TODO RELAY
        Ok("Ni4qf".to_string())
    }

    fn query_orders(
        &self,
        account_id: AccountId,
        orders: Vec<(u32, u32, String)>,
        signature: Signature,
    ) -> RpcResult<Vec<Bytes>> {
        let payload = (account_id, orders, signature).encode();
        let v = self.sign_request(&payload).map_err(|e| {
            RpcError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::ServerError(93101i32).code(),
                "The broker didn't register its key",
                Some(format!("{:?}", e)),
            )))
        })?;
        Ok(vec![])
    }

    fn subscribe_order_events(
        &self,
        mut sink: SubscriptionSink,
        account_id: AccountId,
        signature: Signature,
    ) -> SubscriptionResult {
        Ok(())
    }
}
