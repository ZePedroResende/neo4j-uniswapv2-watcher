use std::fmt::{Debug, Formatter};
use std::{collections::HashSet, sync::Arc};

use crate::uniswapv2_pool_read::SyncEvent;
use crate::uniswapv2_pool_read::{UniswapV2Pair, ERC20};
use ethers::types::U256;
use ethers::{
    providers::{Provider, Ws},
    types::Address,
};
use neo4rs::{query, Graph};

#[derive(Clone)]
pub struct Neo4jStore {
    neo4j: Arc<Graph>,
    nodes: HashSet<String>,
}

impl Debug for Neo4jStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Neo4jStore").finish()
    }
}

impl Neo4jStore {
    pub fn new_store(neo4j: Arc<Graph>) -> Self {
        Self {
            neo4j,
            nodes: HashSet::new(),
        }
    }

    pub async fn update_reserves(&self, event: SyncEvent) -> anyhow::Result<()> {
        let SyncEvent {
            address,
            reserve0,
            reserve1,
        } = event;
        let pool_addr = format!("{:#x}", address);

        let cypher = format!(
            "
            MATCH ()-[r]->()
            WHERE r.pool_address = \"{}\"
            WITH r, CASE r.token
                    WHEN \"0\" THEN '{}'
                    WHEN \"1\" THEN '{}'
                  END AS newCost
            SET r.cost = newCost
            RETURN r ",
            pool_addr, reserve0, reserve1
        );

        let mut result = self.neo4j.execute(query(&cypher)).await?;

        result.next().await?;

        Ok(())
    }

    pub async fn update_pair_metadata(
        &mut self,
        addr: Address,
        provider: Arc<Provider<Ws>>,
    ) -> anyhow::Result<()> {
        let contract = UniswapV2Pair::new(addr, provider.clone());
        let token0 = contract.token_0().call().await?;
        let token1 = contract.token_1().call().await?;

        let contract0 = ERC20::new(token0, provider.clone());
        let contract1 = ERC20::new(token1, provider.clone());

        let symbol0 = contract0.symbol().call().await.map_err(|e| {
            dbg!(addr, token0);
            e
        })?;
        let symbol1 = contract1.symbol().call().await.map_err(|e| {
            dbg!(addr, token1);
            e
        })?;

        let token0 = format!("{:#x}", token0);
        let token1 = format!("{:#x}", token1);
        let pool_addr = format!("{:#x}", addr);

        if !self.nodes.contains(&symbol0) {
            self.create_neo4j_node(token0.to_string().clone(), symbol0.clone())
                .await?;

            self.nodes.insert(symbol0.clone());
        }

        if !self.nodes.contains(&symbol1) {
            self.create_neo4j_node(token1.to_string().clone(), symbol1.clone())
                .await?;

            self.nodes.insert(symbol1.clone());
        }

        self.create_neo4j_relationship(
            pool_addr,
            token0.to_string().clone(),
            token1.to_string().clone(),
            0.into(),
            0.into(),
        )
        .await?;

        Ok(())
    }

    async fn create_neo4j_node(&self, address: String, token: String) -> anyhow::Result<()> {
        let cypher = format!(
            "CREATE (`{}`:Token{{address: \"{}\" , name: \"{}\" }});",
            token.clone(),
            address.to_lowercase().clone(),
            token.clone()
        );

        let mut result = self.neo4j.execute(query(&cypher)).await?;
        result.next().await?;
        Ok(())
    }

    async fn create_neo4j_relationship(
        &self,
        pool_address: String,
        token0_address: String,
        token1_address: String,
        r0: U256,
        r1: U256,
    ) -> anyhow::Result<()> {
        let cypher = format!(
            " MATCH (n {{address: \"{}\" }}), (m {{address: \"{}\"}})
                CREATE (n)-[:UNISWAPV2_POOL {{cost: \"{}\", token: \"0\", pool_address: \"{}\"}} ]->(m)
                CREATE (m)-[:UNISWAPV2_POOL {{cost: \"{}\", token: \"1\", pool_address: \"{}\"}}]->(n)",
            token0_address.to_lowercase(),
            token1_address.to_lowercase(),
            r0,
            pool_address,
            r1,
            pool_address
        );

        let mut result = self.neo4j.execute(query(&cypher)).await?;

        result.next().await?;
        Ok(())
    }
}
