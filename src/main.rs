#[cfg(test)]
#[macro_use]
extern crate hex_literal;

pub mod api;
pub mod blockchain;
pub mod types;
pub mod miner;
pub mod network;
pub mod generator;

use hex_literal::hex;
use blockchain::Blockchain;
use clap::clap_app;
use generator::tx_generator::TxGenerator;
use ring::signature::Ed25519KeyPair;
use smol::channel;
use log::{error, info};
use api::Server as ApiServer;
use types::hash::H256;
use types::key_pair;
use types::transaction::Mempool;
use types::transaction::State;
use std::collections::HashMap;
use std::net;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;

fn main() {
    // parse command line arguments
    let matches = clap_app!(Bitcoin =>
     (version: "0.1")
     (about: "Bitcoin client")
     (@arg verbose: -v ... "Increases the verbosity of logging")
     (@arg peer_addr: --p2p [ADDR] default_value("127.0.0.1:6000") "Sets the IP address and the port of the P2P server")
     (@arg api_addr: --api [ADDR] default_value("127.0.0.1:7000") "Sets the IP address and the port of the API server")
     (@arg known_peer: -c --connect ... [PEER] "Sets the peers to connect to at start")
     (@arg p2p_workers: --("p2p-workers") [INT] default_value("4") "Sets the number of worker threads for P2P server")
    )
    .get_matches();

    // init logger
    let verbosity = matches.occurrences_of("verbose") as usize;
    stderrlog::new().verbosity(verbosity).init().unwrap();
    
    // let blockchain = Blockchain::new();
    
    // let public_key = key_pair::random();
    let blockchain = Blockchain::new();
    let genesis_hash = blockchain.tip();
    let blockchain = Arc::new(Mutex::new(blockchain));

    // Hard code each of the key pair for each node
    let all_public_keys = vec![
        Ed25519KeyPair::from_seed_and_public_key(
            hex!("aac6a03715d753a0da9968a1ab2ee7107395999d92bb028d8af43744474e0401").as_ref(),
            hex!("a776f2f76ba35860d04cfb6123dd7dd419f8f0dc38c6cf422fbcd53820813974").as_ref()
        ).unwrap(),
        Ed25519KeyPair::from_seed_and_public_key(
            hex!("7fa1544a3614bf76fe63671781c6490e21a97ed2124371087b6245aab472dcc6").as_ref(),
            hex!("bd74203c7a28d7de63e02e4bdb7cba4fe033456efce862481edf65f233c067cc").as_ref()
        ).unwrap(),
        Ed25519KeyPair::from_seed_and_public_key(
            hex!("e858bcc3138b3c7d952c1747bba18cf27f8607e7bae5c7952b5a494df16a8253").as_ref(),
            hex!("2581c9f51b3740dd7cacbb691e41989505df86e0a22d93cbd9f527c43bd702d6").as_ref()
        ).unwrap()
    ];
    // let all_public_keys: Vec<Vec<u8>> = match blockchain.lock().unwrap().blocks.values().next() {
    //     // Some((b,_)) => &b.cont.st[0].public_key_vector,
    //     Some((b,_)) => b.clone().cont.st.into_iter().map(|h|h.public_key_vector).collect(),
    //     None => panic!("Getting public key FAILURE!"),
    // };

    // Using the last char of "peer_addr", which can be "0", "1", "2", to determine which above public_key to use for the node
    let public_key_index = matches.value_of("peer_addr").unwrap().chars().last().unwrap().to_digit(10).unwrap() as usize;
    let public_key = &all_public_keys[public_key_index];
    // if matches.value_of("peer_addr").unwrap() == "127.0.0.1:6000" {
    //      public_key = all_public_keys[0];
    // }
    // else if matches.value_of("peer_addr").unwrap() == "127.0.0.1:6001" {
    //     public_key = all_public_keys[1];
    // }
    // else {
    //     public_key = all_public_keys[2];
    // }
    
    // Define the initial state, which has only one entry of ICO in it
    let initial_state = State::new(&all_public_keys[0]);
    // Create the State per block HashMap
    // let mut block_state: HashMap<H256, State> = HashMap::new();
    let block_state: HashMap<H256, State> = HashMap::from([
        (genesis_hash, initial_state),
    ]);
    // block_state.insert(genesis_hash, initial_state);
    let block_state = Arc::new(Mutex::new(block_state));
    let mempool = Arc::new(Mutex::new(Mempool::new()));
    // parse p2p server address
    let p2p_addr = matches
        .value_of("peer_addr")
        .unwrap()
        .parse::<net::SocketAddr>()
        .unwrap_or_else(|e| {
            error!("Error parsing P2P server address: {}", e);
            process::exit(1);
        });

    // parse api server address
    let api_addr = matches
        .value_of("api_addr")
        .unwrap()
        .parse::<net::SocketAddr>()
        .unwrap_or_else(|e| {
            error!("Error parsing API server address: {}", e);
            process::exit(1);
        });

    // create channels between server and worker
    let (msg_tx, msg_rx) = channel::bounded(10000);

    // start the p2p server
    let (server_ctx, server) = network::server::new(p2p_addr, msg_tx).unwrap();
    server_ctx.start().unwrap();

    // Create the transaction generator
    let tx_generator = generator::tx_generator::TxGenerator::new(&server, &mempool, public_key_index, &block_state, &blockchain);
    // let (signal_chan_sender, signal_chan_receiver) = crossbeam::channel::unbounded();

    // start the worker
    let p2p_workers = matches
        .value_of("p2p_workers")
        .unwrap()
        .parse::<usize>()
        .unwrap_or_else(|e| {
            error!("Error parsing P2P workers: {}", e);
            process::exit(1);
        });
    let worker_ctx = network::worker::Worker::new(
        p2p_workers,
        msg_rx,
        &server,
        // Blockchain::new(),
        &blockchain,
        &mempool,
        &block_state,
    );
    worker_ctx.start();

    // start the miner
    let (miner_ctx, miner, finished_block_chan) = miner::new(&blockchain, &mempool);
    let miner_worker_ctx = miner::worker::Worker::new(&server, finished_block_chan, &blockchain, &block_state);
    miner_ctx.start();
    miner_worker_ctx.start();

    // connect to known peers
    if let Some(known_peers) = matches.values_of("known_peer") {
        let known_peers: Vec<String> = known_peers.map(|x| x.to_owned()).collect();
        let server = server.clone();
        thread::spawn(move || {
            for peer in known_peers {
                loop {
                    let addr = match peer.parse::<net::SocketAddr>() {
                        Ok(x) => x,
                        Err(e) => {
                            error!("Error parsing peer address {}: {}", &peer, e);
                            break;
                        }
                    };
                    match server.connect(addr) {
                        Ok(_) => {
                            info!("Connected to outgoing peer {}", &addr);
                            break;
                        }
                        Err(e) => {
                            error!(
                                "Error connecting to peer {}, retrying in one second: {}",
                                addr, e
                            );
                            thread::sleep(time::Duration::from_millis(1000));
                            continue;
                        }
                    }
                }
            }
        });
    }


    // start the API server
    ApiServer::start(
        api_addr,
        &miner,
        &server,
        &blockchain,
        &tx_generator,
        &block_state,
        // &public_key,
    );

    loop {
        std::thread::park();
    }
}
