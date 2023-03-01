use jsonrpsee::{core::RpcResult, proc_macros::rpc};

#[rpc(client, server)]
pub trait BrokerApi {
    #[method(name = "broker_placeOrder")]
    fn place_order(&self) -> RpcResult<String>;

    #[method(name = "broker_cancelOrder")]
    fn cancel_order(&self) -> RpcResult<String>;
}
