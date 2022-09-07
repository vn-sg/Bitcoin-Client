use crate::blockchain::Blockchain;
use crate::network::message::Message;
use crate::network::server::Handle as ServerHandle;
use crate::types::address::Address;
use crate::types::hash::{Hashable, H256};
use crate::types::key_pair;
use crate::types::transaction::{sign, Mempool, SignedTransaction, State, Transaction, Input};
use core::time;
use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use hex_literal::hex;
use log::{debug, info};
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

enum ControlSignal {
    Start(u64),
    Update,
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

#[derive(Clone)]
pub struct TxGenerator {
    chan_sender: Sender<ControlSignal>,
}

struct GeneLoop {
    operating_state: OperatingState,
    chan_receiver: Receiver<ControlSignal>,
    public_key: Ed25519KeyPair,
    mempool: Arc<Mutex<Mempool>>,
    server: ServerHandle,
    addr: Address,
    peer_addrs: (Address, Address),
    block_state: Arc<Mutex<HashMap<H256, State>>>,
    blockchain: Arc<Mutex<Blockchain>>,
}

impl TxGenerator {
    pub fn public_key_gen(index: usize) -> Ed25519KeyPair {
        match index {
            0 => Ed25519KeyPair::from_seed_and_public_key(
                hex!("aac6a03715d753a0da9968a1ab2ee7107395999d92bb028d8af43744474e0401").as_ref(),
                hex!("a776f2f76ba35860d04cfb6123dd7dd419f8f0dc38c6cf422fbcd53820813974").as_ref(),
            )
            .unwrap(),
            1 => Ed25519KeyPair::from_seed_and_public_key(
                hex!("7fa1544a3614bf76fe63671781c6490e21a97ed2124371087b6245aab472dcc6").as_ref(),
                hex!("bd74203c7a28d7de63e02e4bdb7cba4fe033456efce862481edf65f233c067cc").as_ref(),
            )
            .unwrap(),
            2 => Ed25519KeyPair::from_seed_and_public_key(
                hex!("e858bcc3138b3c7d952c1747bba18cf27f8607e7bae5c7952b5a494df16a8253").as_ref(),
                hex!("2581c9f51b3740dd7cacbb691e41989505df86e0a22d93cbd9f527c43bd702d6").as_ref(),
            )
            .unwrap(),

            _ => unimplemented!(),
        }
    }
    pub fn new(
        server: &ServerHandle,
        mp: &Arc<Mutex<Mempool>>,
        // public_key_vector: &Vec<u8>,
        pub_key_index: usize,
        state: &Arc<Mutex<HashMap<H256, State>>>,
        blockchain: &Arc<Mutex<Blockchain>>,
    ) -> Self {
        // let pub_key = ;
        let (signal_chan_sender, signal_chan_receiver) = unbounded();
        let generator = Self {
            // server: server.clone(),
            // mempool: mp.clone(),
            // operating_state: OperatingState::Paused,
            chan_sender: signal_chan_sender,
            // chan_receiver: signal_chan_receiver,
        };

        let pub_key = TxGenerator::public_key_gen(pub_key_index);

        // Keep a copy of the Address of this node itself
        let self_addr = Address::from_public_key_bytes(pub_key.public_key().as_ref());

        let node_count: usize = 3;
        let mut peer_addr: Vec<Address> = Vec::new();
        // iter over peer nodes to retrieve a copy of their Addresses
        for i in 0..node_count {
            if i != pub_key_index {
                peer_addr.push(Address::from_public_key_bytes(
                    TxGenerator::public_key_gen(i).public_key().as_ref(),
                ));
            }
        }

        let mut gen_loop = GeneLoop {
            operating_state: OperatingState::Paused,
            chan_receiver: signal_chan_receiver,
            public_key: pub_key,
            mempool: mp.clone(),
            server: server.clone(),
            addr: self_addr,
            peer_addrs: (peer_addr[0], peer_addr[1]),
            block_state: Arc::clone(state),
            blockchain: Arc::clone(blockchain),
        };

        thread::Builder::new()
            .name("generator".to_string())
            .spawn(move || {
                gen_loop.generator_loop();
            })
            .unwrap();
        info!("TxGenerator initialized into paused mode");

        generator
        // (generator, signal_chan_sender)
    }

    pub fn start(&self, theta: u64) {
        self.chan_sender.send(ControlSignal::Start(theta)).unwrap();
    }
    // pub fn start(&self, theta: u64, pub_key: &Ed25519KeyPair) {
    //     let pub_key = key_pair::random();
    //     self.generator_loop(theta);
    // }
}

impl GeneLoop {
    fn get_tx_balance(&self, state: &State) -> (Vec<Input>, u32) {
        // Initialize the varible 'balance' and 'tx' for consistency
        let mut balance: u32 = 0;
        let mut tx: Vec<Input> = Vec::new();
        // vec![(
        //     hex!("0000000000000000000000000000000000000000000000000000000000000000").into(),
        //     0 as u8,),];
        // let tip = self.blockchain.lock().unwrap().tip().clone();
        // let state = self
        //     .block_state
        //     .lock()
        //     .unwrap()
        //     .get(&tip)
        //     .unwrap()
        //     .clone();

        for ((tx_hash, index), (v, address)) in state.states.iter() {
            if *address == self.addr {
                balance += *v;
                tx.push(Input{prev_trans:*tx_hash, index:*index});
                break;
            }
        }
        (tx, balance)
    }
    fn generator_loop(&mut self) {
        // let mut prev_tx: Option<H256> = None;
        // let mut prev_tip: H256 = hex!("0000000000000000000000000000000000000000000000000000000000000000").into();
        let mut prev_tip: H256 = self.blockchain.lock().unwrap().tip();
        let mut state: State = self.block_state.lock().unwrap().get(&prev_tip).unwrap().clone();
        loop {
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.chan_receiver.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Generator shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Generator starting in continuous mode with theta {}", i);
                            self.operating_state = OperatingState::Run(i);
                        }
                        ControlSignal::Update => {
                            // in paused state, don't need to update
                        }
                    };
                    continue;
                }
                OperatingState::ShutDown => {
                    return;
                }
                _ => match self.chan_receiver.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Generator shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Generator starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                // parent = self.blockchain.lock().unwrap().tip();
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Generator control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // Get the entry in State using the addr of this node itself
            let tip = self.blockchain.lock().unwrap().tip();
            
                    if prev_tip == tip {
                        
                    } else {
                        state = self.block_state.lock().unwrap().get(&tip).unwrap().clone();
                        prev_tip = tip;
                    }

                    match self.get_tx_balance(&state) {
                        (_, 0) => (),
                        (inputs, balance) => {
                            let new_transaction =
                            Transaction::pass_check(&inputs, &balance, &self.peer_addrs);
                            let signature_vector: Vec<u8> =
                                sign(&new_transaction, &self.public_key).as_ref().to_vec();
                            let key_vec = self.public_key.public_key().as_ref().to_vec();

                            let new_signed_transaction = SignedTransaction {
                                transaction: new_transaction,
                                signature_vector: signature_vector,
                                public_key_vector: key_vec,
                            };

                            let tx_hash = new_signed_transaction.hash();
                            if !self.mempool.lock().unwrap().trans.contains_key(&tx_hash) {
                                self.mempool
                                    .lock()
                                    .unwrap()
                                    .trans
                                    .insert(tx_hash, new_signed_transaction.clone());
                            }

                            self.server
                                .broadcast(Message::NewTransactionHashes(vec![tx_hash]));


                            for input in &new_signed_transaction.transaction.input {
                                state.states.remove(&(input.prev_trans, input.index));
                            }
                            for (index, output) in new_signed_transaction.transaction.output.iter().enumerate() {
                                state.states.insert((tx_hash, index as u8), (output.value, output.recipient_addr));
                            }
                            

                        }
                    }
                
            

            // if balance <= 0 {
            //     continue;
            // }

            // self.state.lock().unwrap().states.into_iter().map(|((k1, _),(v1,self.addr)) |s.insert(k,v));

            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }

            // generate
            // insert into mempool
            // broadcast
            // let mut new_trans = ;
            // let mut new_signed_trans = ;

            // let signed_hash = new_signed_trans.hash();
            // self.mempool.lock().unwrap().insert(&signed_hash, new_signed_trans);
            // self.server.broadcast(Message::NewBlockHashes(signed_hash));

            // TODO for student: insert this finished block to blockchain, and broadcast this block hash
        }
    }
}
