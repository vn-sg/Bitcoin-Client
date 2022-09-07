pub mod worker;

use hex::FromHex;
use log::info;

use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use std::time;
use std::sync::{Arc, Mutex};

use std::thread;
use crate::types::block::{Block, Header, Content};
use crate::blockchain::Blockchain;


use crate::types::hash::{H256, Hashable};
use rand::{thread_rng, Rng};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::types::transaction::{SignedTransaction, Mempool, State};
use crate::types::merkle::MerkleTree;


// static DIFFICULTY: &H256 = &([255u8; 32].into());
// static DIFFICULTY: H256 = H256([255u8; 32]);
enum ControlSignal {
    Start(u64), // the number controls the lambda of interval between block generation
    Update, // update the block in mining, it may due to new blockchain tip or new transaction
    Exit,
}

enum OperatingState {
    Paused,
    Run(u64),
    ShutDown,
}

pub struct Context {
    /// Channel for receiving control signal
    control_chan: Receiver<ControlSignal>,
    operating_state: OperatingState,
    finished_block_chan: Sender<Block>,
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mutex<Mempool>>, 
}
#[derive(Clone)]
pub struct Handle {
    /// Channel for sending signal to the miner thread
    control_chan: Sender<ControlSignal>,
}

pub fn new(blockchain: &Arc<Mutex<Blockchain>>, mempool: &Arc<Mutex<Mempool>>) -> (Context, Handle, Receiver<Block>) {
    let (signal_chan_sender, signal_chan_receiver) = unbounded();
    let (finished_block_sender, finished_block_receiver) = unbounded();

    
    let ctx = Context {
        control_chan: signal_chan_receiver,
        operating_state: OperatingState::Paused,
        finished_block_chan: finished_block_sender,
        blockchain: Arc::clone(blockchain),
        mempool: Arc::clone(mempool),
    };

    let handle = Handle {
        control_chan: signal_chan_sender,
    };


    (ctx, handle, finished_block_receiver)
}

#[cfg(any(test,test_utilities))]
fn test_new() -> (Context, Handle, Receiver<Block>) {
    let blockchain = &Arc::new(Mutex::new(Blockchain::new()));
    let mempool = &Arc::new(Mutex::new(Mempool::new()));
    new(&blockchain, &mempool)
}

impl Handle {
    pub fn exit(&self) {
        self.control_chan.send(ControlSignal::Exit).unwrap();
    }

    pub fn start(&self, lambda: u64) {
        self.control_chan
            .send(ControlSignal::Start(lambda))
            .unwrap();
    }

    pub fn update(&self) {
        self.control_chan.send(ControlSignal::Update).unwrap();
    }
}

impl Context {
    pub fn start(mut self) {
        thread::Builder::new()
            .name("miner".to_string())
            .spawn(move || {
                self.miner_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

    fn miner_loop(&mut self) {
        // main mining loop
        // let mut parent = self.blockchain.lock().unwrap().tip();
        let mut to_remove: Vec<H256> = Vec::new();
        let mut transaction: Vec<SignedTransaction> = Vec::new();
        let mut count = 0;
        loop {
            // check and react to control signals
            match self.operating_state {
                OperatingState::Paused => {
                    let signal = self.control_chan.recv().unwrap();
                    match signal {
                        ControlSignal::Exit => {
                            info!("Miner shutting down");
                            self.operating_state = OperatingState::ShutDown;
                        }
                        ControlSignal::Start(i) => {
                            info!("Miner starting in continuous mode with lambda {}", i);
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
                _ => match self.control_chan.try_recv() {
                    Ok(signal) => {
                        match signal {
                            ControlSignal::Exit => {
                                info!("Miner shutting down");
                                self.operating_state = OperatingState::ShutDown;
                            }
                            ControlSignal::Start(i) => {
                                info!("Miner starting in continuous mode with lambda {}", i);
                                self.operating_state = OperatingState::Run(i);
                            }
                            ControlSignal::Update => {
                                // parent = self.blockchain.lock().unwrap().tip();
                            }
                        };
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => panic!("Miner control channel detached"),
                },
            }
            if let OperatingState::ShutDown = self.operating_state {
                return;
            }

            // TODO for student: actual mining, create a block
            // TODO for student: if block mining finished, you can have something like: self.finished_block_chan.send(block.clone()).expect("Send finished block error");
            

            // let parent = cur_blockchain.tip();
            let mut rng = rand::thread_rng();
            let nonce: u32= rng.gen();

            let current_time_mili = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(n) => n.as_millis(),
                Err(_) => panic!("SystemTime before UNIX EPOCH!"),
            };

            
            // let mut cur_mempool = self.mempool.lock().unwrap();
            let block_size = 50;

            
            if to_remove.len() == 0 {
                for (h, trans) in self.mempool.lock().unwrap().trans.iter() {

                    if to_remove.len() >= block_size {
                        break;
                    }
                    // if tx_exist_check(&self.blockchain, h) {
                    transaction.push(trans.clone());
                    // }
                    to_remove.push(h.clone());
                }
            }

            // Create Merkle root
            let tree = MerkleTree::new(&transaction);
            // let local: Vec<SignedTransaction> = Vec::new();
            // let tree = MerkleTree::new(&transaction);
            let root = tree.root();
            
            // set difficulty to be 0xff...fff
            // let diff: H256 = [255u8; 32].into();

            // set new difficulty
            let diff: H256 = hex_literal::hex!("000ff93a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into();

            // Retrieve the tip of the blockchain and set that as the parent
            // parent = self.blockchain.lock().unwrap().tip();
            let h = Header{ parent: self.blockchain.lock().unwrap().tip(), nonce: nonce, difficulty:diff, timestamp: current_time_mili, merkle_root: root };
            // let c = transaction.clone();
            let block = Block{head: h, cont: Content{ st: transaction.clone()}};
            // let block = Block{head: h, cont: Content{ st: local}};
            
            if block.hash() <= diff {
                self.finished_block_chan.send(block.clone()).expect("Send finished block error");
                // parent = block.hash();

                // to_remove.iter().map(| item | self.mempool.lock().unwrap().trans.remove(item));
                for item in to_remove.iter() {
                    self.mempool.lock().unwrap().trans.remove(item);
                }
                to_remove.clear();
                transaction.clear();
                // transaction = Vec::new();
                // let transaction: Vec<SignedTransaction> = Vec::new();
            }
            count += 1;
            // println!("CYCLE: {}\n", count);


            if let OperatingState::Run(i) = self.operating_state {
                if i != 0 {
                    let interval = time::Duration::from_micros(i as u64);
                    thread::sleep(interval);
                }
            }
        }
    }
}


pub fn tx_exist_check(blockchain: &Arc<Mutex<Blockchain>>, tx: &H256) -> bool {
    let tip = blockchain.lock().unwrap().tip();
    let (mut block, mut height) = blockchain.lock().unwrap().blocks.get(&tip).unwrap().clone();
    for signed_transaction in block.cont.st.iter() {
        if *tx == signed_transaction.hash() {
            return false;
        }
    }
    // hashes_string.push(block.cont.st.into_iter().map(|t|t.hash().to_string()).collect());
    let mut next_hash = block.head.parent;
    while height >= 1 {
        // println!("{}", height);
        block = blockchain.lock().unwrap().blocks.get(&next_hash).unwrap().0.clone();
        for signed_transaction in block.cont.st.iter() {
            if *tx == signed_transaction.hash() {
                return false;
            }
        }
        next_hash = block.head.parent;
        height -= 1;
    }
    true
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod test {
    use ntest::timeout;
    use crate::types::hash::Hashable;

    #[test]
    #[timeout(60000)]
    fn miner_three_block() {
        let (miner_ctx, miner_handle, finished_block_chan) = super::test_new();
        miner_ctx.start();
        miner_handle.start(0);
        let mut block_prev = finished_block_chan.recv().unwrap();
        for _ in 0..1000{
            let block_next = finished_block_chan.recv().unwrap();
            assert_eq!(block_prev.hash(), block_next.get_parent());
            block_prev = block_next;
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST