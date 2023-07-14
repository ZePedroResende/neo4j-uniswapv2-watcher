use std::convert::TryFrom;
use std::fmt::Debug;
use std::sync::Arc;

use ethers::providers::{Authorization, Http, Provider, Ws};
use url::Url;

use crate::neo4j_store::Neo4jStore;
use clap::Parser;
use neo4rs::{query, Graph};

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, env = "EVM_ENDPOINT_RPC")]
    pub rpc: String,
    #[arg(long, env = "EVM_ENDPOINT_WSS")]
    pub wss: String,
    #[arg(long, env = "EVM_ENDPOINT_USER")]
    pub username: Option<String>,
    #[arg(long, env = "EVM_ENDPOINT_PASS")]
    pub password: Option<String>,
    #[arg(long, env = "EVM_CHAIN")]
    pub chain: String,
}

impl Default for Args {
    fn default() -> Self {
        Self::parse()
    }
}

pub type HttpProvider = Provider<Http>;

#[derive(Clone, Debug)]
pub struct Context {
    pub http: Arc<HttpProvider>,
    pub wss: Arc<Provider<Ws>>,
    pub pair_store: Neo4jStore,
    pub chain: String,
}

impl Context {
    pub async fn from_args(args: Args) -> Self {
        let provider: Provider<Http>;
        let ws_provider: Provider<Ws>;

        if args.username.is_some() {
            let url = Url::parse(&args.rpc).expect("Failed to parse http provider url");

            let username = args.username.expect("missing username");
            let password = args.password.expect("missing password");

            let basic_auth = Authorization::basic(&username, &password);

            let http: Http =
                Http::new_with_auth(url, basic_auth.clone()).expect("Failed auth with http");

            provider = Provider::new(http);

            ws_provider = Provider::<Ws>::connect_with_auth(args.wss, basic_auth)
                .await
                .expect("Failed auth with ws");
        } else {
            provider = Provider::<Http>::try_from(args.rpc).unwrap();
            ws_provider = Provider::<Ws>::connect(args.wss).await.unwrap();
        };

        let wss = Arc::new(ws_provider);

        let uri = "127.0.0.1:7687";
        let user = "neo4j";
        let pass = "password";
        let graph = Arc::new(Graph::new(uri, user, pass).await.unwrap());

        assert!(graph.run(query("RETURN 1")).await.is_ok());

        let pair_store = Neo4jStore::new_store(graph);
        let chain = args.chain;

        let http = Arc::new(provider);

        Self {
            http,
            wss,
            pair_store,
            chain,
        }
    }
}
