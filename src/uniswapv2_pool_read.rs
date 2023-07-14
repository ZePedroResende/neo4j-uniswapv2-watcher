use ethers::prelude::abigen;
use ethers::{
    abi,
    abi::ParamType,
    providers::{Middleware, StreamExt},
    types::{Address, BlockNumber, Filter, Log, ValueOrArray, H256, U256},
    utils::keccak256,
};

use crate::context::Context;

use std::collections::HashSet;

abigen!(UniswapV2Pair, "abis/UniswapV2Pair.json");

abigen!(ERC20, "abis/ERC20.json");

#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub address: Address,
    pub reserve0: U256,
    pub reserve1: U256,
}

impl TryInto<SyncEvent> for Log {
    type Error = ();

    fn try_into(self) -> Result<SyncEvent, Self::Error> {
        let reserves = abi::decode(&[ParamType::Uint(112), ParamType::Uint(112)], &self.data);

        if reserves.is_err() {
            return Err(());
        }
        let reserves = reserves.unwrap();

        let (reserve0, reserve1) = (
            reserves[0].clone().into_uint().unwrap(),
            reserves[1].clone().into_uint().unwrap(),
        );

        Ok(SyncEvent {
            address: self.address,
            reserve0,
            reserve1,
        })
    }
}

/// listens to uniswapv2 Sync events
pub async fn listen_prices(ctx: Context) -> anyhow::Result<()> {
    let provider = ctx.wss.clone();

    let mut pair_store = ctx.pair_store.clone();

    let last_block = provider
        .get_block(BlockNumber::Latest)
        .await?
        .unwrap()
        .number
        .unwrap();

    let prices_filter = Filter::new()
        .from_block(last_block)
        .topic0(ValueOrArray::Value(H256::from(keccak256(
            "Sync(uint112,uint112)",
        ))));

    let mut known_pairs: HashSet<Address> = HashSet::new();
    let mut stream = provider.subscribe_logs(&prices_filter).await?;

    while let Some(log) = stream.next().await {
        let event: SyncEvent = log.try_into().unwrap();

        if !known_pairs.contains(&event.address) {
            known_pairs.insert(event.address);

            let provider = provider.clone();

            let _ = pair_store
                .update_pair_metadata(event.address, provider)
                .await
                .map_err(|e| {
                    dbg!(e);
                })
                .map(|_| async {
                    pair_store.update_reserves(event).await.unwrap();
                });
        }
    }

    Ok(())
}
