//mod sync;
//mod graph;
mod context;
mod neo4j_store;
mod uniswapv2_pool_read;

use crate::context::{Args, Context};
use crate::uniswapv2_pool_read::listen_prices;
use anyhow::{Ok, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let ctx = Context::from_args(Args::default()).await;

    let _ = listen_prices(ctx).await;

    Ok(())
}
