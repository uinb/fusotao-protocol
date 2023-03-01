use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use sp_rpc::number::NumberOrHex;

#[rpc(client, server)]
pub trait TokenApi<BlockHash, Account, Balance> {
    #[method(name = "token_freeBalance")]
    fn free_balance(&self, who: Account, at: Option<BlockHash>) -> RpcResult<Balance>;

    #[method(name = "token_totalBalance")]
    fn total_balance(&self, who: Account, at: Option<BlockHash>) -> RpcResult<Balance>;
}
