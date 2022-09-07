use crate::blockchain::Blockchain;
use crate::generator::tx_generator::TxGenerator;
use crate::miner::Handle as MinerHandle;
use crate::network::message::Message;
use crate::network::server::Handle as NetworkServerHandle;
use crate::types::hash::Hashable;
use crate::types::transaction::State;
use crate::types::address::Address;
use crate::H256;

use serde::Serialize;

use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server as HTTPServer;
use url::Url;

pub struct Server {
    handle: HTTPServer,
    miner: MinerHandle,
    network: NetworkServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    tx_generator: TxGenerator,
    block_state: Arc<Mutex<HashMap<H256, State>>>,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
}

macro_rules! respond_result {
    ( $req:expr, $success:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let payload = ApiResponse {
            success: $success,
            message: $message.to_string(),
        };
        let resp = Response::from_string(serde_json::to_string_pretty(&payload).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}
macro_rules! respond_json {
    ( $req:expr, $message:expr ) => {{
        let content_type = "Content-Type: application/json".parse::<Header>().unwrap();
        let resp = Response::from_string(serde_json::to_string(&$message).unwrap())
            .with_header(content_type);
        $req.respond(resp).unwrap();
    }};
}

impl Server {
    pub fn start(
        addr: std::net::SocketAddr,
        miner: &MinerHandle,
        network: &NetworkServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
        tx_generator: &TxGenerator,
        block_state: &Arc<Mutex<HashMap<H256, State>>>,
        // pub_key: &Ed25519KeyPair,
    ) {
        let handle = HTTPServer::http(&addr).unwrap();
        let server = Self {
            handle,
            miner: miner.clone(),
            network: network.clone(),
            blockchain: Arc::clone(blockchain),
            tx_generator: tx_generator.clone(),
            block_state: Arc::clone(block_state),
        };
        thread::spawn(move || {
            for req in server.handle.incoming_requests() {
                let miner = server.miner.clone();
                let network = server.network.clone();
                let blockchain = Arc::clone(&server.blockchain);
                let tx_generator = server.tx_generator.clone();
                let block_state = Arc::clone(&server.block_state);
                thread::spawn(move || {
                    // a valid url requires a base
                    let base_url = Url::parse(&format!("http://{}/", &addr)).unwrap();
                    let url = match base_url.join(req.url()) {
                        Ok(u) => u,
                        Err(e) => {
                            respond_result!(req, false, format!("error parsing url: {}", e));
                            return;
                        }
                    };
                    match url.path() {
                        "/miner/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let lambda = match params.get("lambda") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing lambda");
                                    return;
                                }
                            };
                            let lambda = match lambda.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing lambda: {}", e)
                                    );
                                    return;
                                }
                            };
                            miner.start(lambda);
                            respond_result!(req, true, "ok");
                        }
                        "/tx-generator/start" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let theta = match params.get("theta") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing theta");
                                    return;
                                }
                            };
                            let theta = match theta.parse::<u64>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing theta: {}", e)
                                    );
                                    return;
                                }
                            };
                            // tx_generator.start(theta);
                            // thread::spawn(move || {
                            tx_generator.start(theta);
                            // });
                            respond_result!(req, true, "ok!");
                        }
                        "/network/ping" => {
                            network.broadcast(Message::Ping(String::from("Test ping")));
                            respond_result!(req, true, "ok");
                        }
                        "/blockchain/longest-chain" => {
                            let blockchain = blockchain.lock().unwrap();
                            let v = blockchain.all_blocks_in_longest_chain();
                            let v_string: Vec<String> =
                                v.into_iter().map(|h| h.to_string()).collect();
                            respond_json!(req, v_string);
                        }
                        "/blockchain/longest-chain-tx" => {
                            // println!("GETING INTO TX");
                            // Get the longest chain and put all TXs in it to JSON format
                            let mut hashes_string: Vec<Vec<String>> = Vec::new();
                            let tip = blockchain.lock().unwrap().tip();

                            let (mut block, mut height) =
                                blockchain.lock().unwrap().blocks.get(&tip).unwrap().clone();

                            hashes_string.push(
                                block
                                    .cont
                                    .st
                                    .into_iter()
                                    .map(|t| t.hash().to_string())
                                    .collect(),
                            );
                            let mut next_hash = block.head.parent;
                            while height >= 1 {
                                // println!("{}", height);
                                block = blockchain
                                    .lock()
                                    .unwrap()
                                    .blocks
                                    .get(&next_hash)
                                    .unwrap()
                                    .0
                                    .clone();
                                hashes_string.push(
                                    block
                                        .cont
                                        .st
                                        .into_iter()
                                        .map(|t| t.hash().to_string())
                                        .collect(),
                                );
                                next_hash = block.head.parent;
                                height -= 1;
                            }

                            hashes_string.reverse();
                            // println!("!!!!!!!!!!!!");
                            // println!("{:?}",hashes_string);
                            respond_json!(req, hashes_string);
                        }
                        "/blockchain/longest-chain-tx-count" => {
                            // unimplemented!()
                            respond_result!(req, false, "unimplemented!");
                        }
                        "/blockchain/state" => {
                            let params = url.query_pairs();
                            let params: HashMap<_, _> = params.into_owned().collect();
                            let block_index = match params.get("block") {
                                Some(v) => v,
                                None => {
                                    respond_result!(req, false, "missing block no.");
                                    return;
                                }
                            };
                            let block_index = match block_index.parse::<u32>() {
                                Ok(v) => v,
                                Err(e) => {
                                    respond_result!(
                                        req,
                                        false,
                                        format!("error parsing block no: {}", e)
                                    );
                                    return;
                                }
                            };
                            // let mut current_state: Vec<((H256, u8, u32, Address))> = Vec::new();

                            // for ((k1, k2), (v1,v2)) in state.lock().unwrap().states.iter() {
                            //     current_state.push((k1.clone(), k2.clone(), v1.clone(), v2.clone()));
                            // }

                            // let states_clone = block_state
                            //     .lock()
                            //     .unwrap()
                            //     .get(&blockchain.lock().unwrap().tip())
                            //     .unwrap()
                            //     .clone();
                        
                            // let mut longest_chain: Vec<H256> = Vec::new();
                            let tip = blockchain.lock().unwrap().tip();

                            let (mut block, mut height) =
                                blockchain.lock().unwrap().blocks.get(&tip).unwrap().clone();

                            // longest_chain.push(tip);
                            let mut next_hash = tip;
                            while height > block_index{
                                // println!("{}", height);
                                // longest_chain.push(next_hash);
                                next_hash = block.head.parent;
                                block = blockchain
                                .lock()
                                .unwrap()
                                .blocks
                                .get(&next_hash)
                                .unwrap()
                                .0
                                .clone();
                                height -= 1;
                            }
                            // longest_chain.reverse();

                            // let target_block: H256 = longest_chain[block_index];
                            let states_clone: State = block_state.lock().unwrap().get(&next_hash).unwrap().clone();

                            let current_state: Vec<(String, String, String, String)> = 
                                states_clone
                                    .states
                                    .into_iter()
                                    .map(|((k1, k2), (v1,v2)) | (k1.to_string(), k2.to_string(), v1.to_string(), v2.to_string()) )
                                    .collect();

                            respond_json!(req, current_state);
                        }

                        _ => {
                            let content_type =
                                "Content-Type: application/json".parse::<Header>().unwrap();
                            let payload = ApiResponse {
                                success: false,
                                message: "endpoint not found".to_string(),
                            };
                            let resp = Response::from_string(
                                serde_json::to_string_pretty(&payload).unwrap(),
                            )
                            .with_header(content_type)
                            .with_status_code(404);
                            req.respond(resp).unwrap();
                        }
                    }
                });
            }
        });
        info!("API server listening at {}", &addr);
    }
}
