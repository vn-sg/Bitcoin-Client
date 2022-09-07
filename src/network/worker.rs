use super::message::Message;
use super::peer;
use super::server::Handle as ServerHandle;
use crate::blockchain::Blockchain;
use crate::types::address::Address;
use crate::types::block::Block;
use crate::types::hash::{Hashable, H256};
use crate::types::transaction::{verify, Mempool, SignedTransaction, State};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use log::{debug, error, warn};

use std::thread;

#[cfg(any(test, test_utilities))]
use super::peer::TestReceiver as PeerTestReceiver;
#[cfg(any(test, test_utilities))]
use super::server::TestReceiver as ServerTestReceiver;
#[derive(Clone)]
pub struct Worker {
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    mempool: Arc<Mutex<Mempool>>,
    block_state: Arc<Mutex<HashMap<H256, State>>>,
}
pub struct OrphanBuffer(Vec<Block>);

impl Worker {
    pub fn new(
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
        block_chain: &Arc<Mutex<Blockchain>>,
        mempool: &Arc<Mutex<Mempool>>,
        block_state: &Arc<Mutex<HashMap<H256, State>>>,
    ) -> Self {
        Self {
            msg_chan: msg_src,
            num_worker,
            server: server.clone(),
            blockchain: Arc::clone(&block_chain),
            mempool: Arc::clone(&mempool),
            block_state: Arc::clone(&block_state),
        }
    }

    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    // When a block passes check and its parent is present, Check the states if TXs are valid
    // Returns true and update block_state if all TXs are valid, otherwise return false and do nothing
    fn check_tx_state(&self, parent_hash: &H256, signed_txs: &Vec<SignedTransaction>, block_hash: &H256) -> bool {
        // verify all the SignedTransactions inside the block
        let mut state = self
        .block_state
        .lock()
        .unwrap()
        .get(parent_hash)
        .unwrap()
        .clone();

        for st in signed_txs {
            let tx = st.transaction.clone();
            let owner_public_key = st.public_key_vector.as_ref();
            match verify(&tx, owner_public_key, st.signature_vector.as_ref()) {
                true => (),
                false => {
                    return false;
                }
            }
            // let state = 

            let mut sum_input: u32 = 0;
            for i in &tx.input {
                match state.states.get(&(i.prev_trans, i.index)) {
                    Some((value, recipient)) => {
                        // Comparing the Address of the signder and the Address of recipient of input
                        // If not match, abort
                        if *recipient != Address::from_public_key_bytes(owner_public_key) {
                            return false;
                        }

                        sum_input += value;
                    }
                    None => {
                        return false;
                    }
                }
            }

            let sum_output: u32 = tx.output.iter().map(|x| x.value as u32).sum();

            if sum_input < sum_output {
                return false;
            }

            // tx.input.iter().map(|input | state.states.remove(&(input.prev_trans, input.index)));
            // Update state after TX passes the check
            for input in &tx.input {
                state.states.remove(&(input.prev_trans, input.index));
            }
            for (index, output) in tx.output.iter().enumerate() {
                state.states.insert((st.hash(), index as u8), (output.value, output.recipient_addr));
            }
        }

        // Insert the entry for the block and the state after executing it
        self.block_state.lock().unwrap().insert(*block_hash, state);
        // Return true
        true
    }

    fn worker_loop(&self) {
        let mut orphan_blocks = OrphanBuffer(Vec::new());
        loop {
            let result = smol::block_on(self.msg_chan.recv());
            if let Err(e) = result {
                error!("network worker terminated {}", e);
                break;
            }
            let msg = result.unwrap();
            let (msg, mut peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                Message::NewBlockHashes(hashes) => {
                    let chain = self.blockchain.lock().unwrap();
                    let mut block_hashes: Vec<H256> = Vec::new();
                    for hash in hashes {
                        if !chain.blocks.contains_key(&hash) {
                            block_hashes.push(hash);
                        }
                    }
                    if block_hashes.len() > 0 {
                        peer.write(Message::GetBlocks(block_hashes));
                    }
                }
                Message::GetBlocks(block_hashes) => {
                    let chain = self.blockchain.lock().unwrap();
                    let mut blocks: Vec<Block> = Vec::new();
                    let mut has_blocks = true;
                    for hash in block_hashes {
                        if chain.blocks.contains_key(&hash) {
                            blocks.push(chain.blocks.get(&hash).unwrap().0.clone());
                        } else {
                            has_blocks = false;
                            break;
                        }
                    }
                    if has_blocks {
                        peer.write(Message::Blocks(blocks));
                    }
                }
                Message::Blocks(blocks) => {
                    // let mut chain = self.blockchain.lock().unwrap();
                    let mut new_blocks: Vec<H256> = Vec::new();
                    let mut n: usize = 0;
                    for block in blocks {
                        let block_hash = block.hash();
                        let parent_hash = block.head.parent;
                        if block_hash <= block.head.difficulty {
                            if !self
                                .blockchain
                                .lock()
                                .unwrap()
                                .blocks
                                .contains_key(&block_hash)
                            {
                                // Parent check
                                if self
                                    .blockchain
                                    .lock()
                                    .unwrap()
                                    .blocks
                                    .contains_key(&parent_hash)
                                {
                                    // Check if the difficulty equals the parent block's difficulty
                                    if block.head.difficulty
                                        == self
                                            .blockchain
                                            .lock()
                                            .unwrap()
                                            .blocks
                                            .get(&block.head.parent)
                                            .unwrap()
                                            .0
                                            .head
                                            .difficulty
                                    {
                                        // let mut valid_transactions = true;
                                        if self.check_tx_state(&parent_hash, &block.cont.st, &block_hash) {
                                            self.blockchain.lock().unwrap().insert(&block);
                                            new_blocks.push(block_hash);
                                        }
                                        // // verify all the SignedTransactions inside the block
                                        // for st in &block.cont.st {
                                        //     match verify(&st.transaction.clone(), st.public_key_vector.as_ref(), st.signature_vector.as_ref()) {
                                        //         true => continue,
                                        //         false => {
                                        //             valid_transactions = false;
                                        //             break;
                                        //         },
                                        //     }
                                        // }
                                        // if valid_transactions {
                                        //     self.blockchain.lock().unwrap().insert(&block);
                                        //     new_blocks.push(block_hash);
                                        // }
                                    }

                                    loop {
                                        if n >= new_blocks.len() {
                                            break;
                                        }

                                        let mut i: usize = 0;
                                        loop {
                                            if i >= orphan_blocks.0.len() {
                                                break;
                                            }
                                            if orphan_blocks.0[i].head.parent == new_blocks[n] {
                                                let block_to_unorphan = orphan_blocks.0.remove(i);
                                                if block_to_unorphan.head.difficulty
                                                    == self
                                                        .blockchain
                                                        .lock()
                                                        .unwrap()
                                                        .blocks
                                                        .get(&new_blocks[n])
                                                        .unwrap()
                                                        .0
                                                        .head
                                                        .difficulty
                                                {
                                                    let block_to_unorphan_hash = block_to_unorphan.hash();
                                                    if self.check_tx_state(&block_to_unorphan.head.parent, &block_to_unorphan.cont.st, &block_to_unorphan_hash) {
                                                        // self.blockchain.lock().unwrap().insert(&block);
                                                        // new_blocks.push(block_hash);
                                                        self.blockchain
                                                        .lock()
                                                        .unwrap()
                                                        .insert(&block_to_unorphan);
                                                        new_blocks.push(block_to_unorphan_hash);
                                                    }
                                                    
                                                }
                                            } else {
                                                i += 1;
                                            }
                                        }
                                        n += 1;

                                        // for orphan in orphan_blocks.0.iter() {
                                        //     if orphan.head.parent == new_blocks[n] {
                                        //     }
                                        // }
                                    }
                                }
                                //Parent check fails
                                else {
                                    orphan_blocks.0.push(block.clone());
                                    let parent_hash = orphan_blocks.0.last().unwrap().head.parent;
                                    // let parent_hash = block.clone().head.parent;
                                    peer.write(Message::GetBlocks(vec![parent_hash]));
                                }

                                // // check state

                                // let mut valid = true;
                                // let mut cur_state = self
                                //     .block_state
                                //     .lock()
                                //     .unwrap()
                                //     .get(&block.head.parent)
                                //     .unwrap()
                                //     .clone();
                                // let block_trans = block.clone().cont.st;
                                // for tx in block_trans {
                                //     let trans = tx.clone().transaction;
                                //     let pubkey = tx.clone().public_key_vector;
                                //     let key_hash: H256 = ring::digest::digest(
                                //         &ring::digest::SHA256,
                                //         pubkey.as_ref(),
                                //     )
                                //     .into();
                                //     let input = tx.clone().transaction.input;
                                //     let output = tx.clone().transaction.output;
                                //     let mut input_value = 0;
                                //     for each_in in input {
                                //         let prev = each_in.prev_trans;
                                //         let prev_index = each_in.index;
                                //         if cur_state.states.contains_key(&(prev, prev_index)) {
                                //             let recipient: Address =
                                //                 cur_state.states[&(prev, prev_index)].1;
                                //             let _recipient: Address =
                                //                 Address::from_public_key_bytes(key_hash.as_ref());
                                //             if recipient != _recipient {
                                //                 valid = false;
                                //                 break;
                                //             }
                                //             input_value += cur_state.states[&(prev, prev_index)].0;
                                //         } else {
                                //             valid = false;
                                //             break;
                                //         }
                                //     }
                                //     let mut output_value = 0;
                                //     for out in output {
                                //         output_value += out.value;
                                //     }
                                //     if input_value <= output_value {
                                //         valid = false;
                                //     }
                                // }
                                // if !valid {
                                //     break;
                                // }

                                // // block state update
                                // for tx in block.clone().cont.st {
                                //     cur_state.update(&tx);
                                // }

                                // // update mempool
                                // let mut memp = self.mempool.lock().unwrap();
                                // let mut memp_copy = memp.trans.clone();
                                // let mut to_remove = Vec::new();

                                // let memp_keys = memp_copy.keys();
                                // for key in memp_keys {
                                //     let value = &memp.trans[&key];
                                //     for i in 0..value.transaction.input.len() {
                                //         let to_find = (
                                //             memp.trans[&key].transaction.input[i].prev_trans,
                                //             memp.trans[&key].transaction.input[i].index,
                                //         );
                                //         if !cur_state.states.contains_key(&to_find) {
                                //             to_remove.push(key);
                                //             break;
                                //         }
                                //     }
                                // }

                                // for key in to_remove {
                                //     memp.trans.remove(&key);
                                // }
                                // self.block_state
                                //     .lock()
                                //     .unwrap()
                                //     .insert(block.hash(), cur_state.clone());
                            }
                        }
                    }

                    if new_blocks.len() > 0 {
                        self.server.broadcast(Message::NewBlockHashes(new_blocks));
                    }
                }
                Message::NewTransactionHashes(hashes) => {
                    let mut new_trans_hashes: Vec<H256> = Vec::new();

                    for hash in hashes {
                        // let has_key = self.mempool.lock().unwrap().trans.contains_key(&hash);
                        if true {
                            new_trans_hashes.push(hash);
                        }
                    }
                }

                Message::GetTransactions(transaction_hashes) => {
                    // Of type Vec<H256>
                    // let mempool = self.mempool.lock().unwrap();
                    let mut trans: Vec<SignedTransaction> = Vec::new();
                    let mut has_trans = true;
                    for hash in transaction_hashes {
                        if self.mempool.lock().unwrap().trans.contains_key(&hash) {
                            trans.push(
                                self.mempool
                                    .lock()
                                    .unwrap()
                                    .trans
                                    .get(&hash)
                                    .unwrap()
                                    .clone(),
                            );
                        } else {
                            has_trans = false;
                            break;
                        }
                    }
                    if has_trans {
                        peer.write(Message::Transactions(trans));
                    }
                }

                Message::Transactions(transactions) => {
                    // of type Vec<SignedTransaction>
                    // let mempool = self.mempool.lock().unwrap();
                    let mut new_trans = Vec::new();
                    for trans in transactions {
                        // let sign = trans.signature_vector.clone();
                        // let public_key = trans.public_key_vector.clone();
                        let verify_ = verify(
                            &trans.transaction.clone(),
                            trans.public_key_vector.as_ref(),
                            trans.signature_vector.as_ref(),
                        );
                        if !verify_ {
                            println!("wrong signature");
                            continue;
                        }
                        let h = trans.hash();
                        if !self.mempool.lock().unwrap().trans.contains_key(&h) {
                            new_trans.push(h);

                            self.mempool
                                .lock()
                                .unwrap()
                                .trans
                                .insert(trans.hash(), trans.clone());
                        }
                    }

                    if new_trans.len() > 0 {
                        self.server
                            .broadcast(Message::NewTransactionHashes(new_trans));
                    }
                }

                _ => unimplemented!(),
            }
        }
    }
}

#[cfg(any(test, test_utilities))]
struct TestMsgSender {
    s: smol::channel::Sender<(Vec<u8>, peer::Handle)>,
}
#[cfg(any(test, test_utilities))]
impl TestMsgSender {
    fn new() -> (
        TestMsgSender,
        smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    ) {
        let (s, r) = smol::channel::unbounded();
        (TestMsgSender { s }, r)
    }

    fn send(&self, msg: Message) -> PeerTestReceiver {
        let bytes = bincode::serialize(&msg).unwrap();
        let (handle, r) = peer::Handle::test_handle();
        smol::block_on(self.s.send((bytes, handle))).unwrap();
        r
    }
}
// #[cfg(any(test,test_utilities))]
// /// returns two structs used by tests, and an ordered vector of hashes of all blocks in the blockchain
// fn generate_test_worker_and_start() -> (TestMsgSender, ServerTestReceiver, Vec<H256>) {
//     let (server, server_receiver) = ServerHandle::new_for_test();
//     let (test_msg_sender, msg_chan) = TestMsgSender::new();
//     // let blockchain = Blockchain::new();
//     let blockchain = Arc::new(Mutex::new(Blockchain::new()));
//     let mut block_hashes: Vec<H256> = Vec::new();
//     for hash in blockchain.lock().unwrap().blocks.keys() {
//         block_hashes.push(*hash);
//             // Vec<H256>= collect::<Vec<H256>>();
//     }
//     let mempool = Arc::new(Mutex::new(Mempool::new()));

//     let state = HashMap::new();

//     // state.insert(k, State::new());
//     // let block_state = Arc::new(Mutex::new(state));

//     let worker = Worker::new(1, msg_chan, &server, &blockchain, &mempool, &block_sate);
//     worker.start();
//     (test_msg_sender, server_receiver, block_hashes)
// }

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

// #[cfg(test)]
// mod test {
//     use ntest::timeout;
//     use crate::types::block::generate_random_block;
//     use crate::types::hash::Hashable;

//     use super::super::message::Message;
//     use super::generate_test_worker_and_start;

//     #[test]
//     #[timeout(60000)]
//     fn reply_new_block_hashes() {
//         let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
//         let random_block = generate_random_block(v.last().unwrap());
//         let mut peer_receiver = test_msg_sender.send(Message::NewBlockHashes(vec![random_block.hash()]));
//         let reply = peer_receiver.recv();
//         if let Message::GetBlocks(v) = reply {
//             assert_eq!(v, vec![random_block.hash()]);
//         } else {
//             panic!();
//         }
//     }
//     #[test]
//     #[timeout(60000)]
//     fn reply_get_blocks() {
//         let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
//         let h = v.last().unwrap().clone();
//         let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(vec![h.clone()]));
//         let reply = peer_receiver.recv();
//         if let Message::Blocks(v) = reply {
//             assert_eq!(1, v.len());
//             assert_eq!(h, v[0].hash())
//         } else {
//             panic!();
//         }
//     }
//     #[test]
//     #[timeout(60000)]
//     fn reply_blocks() {
//         let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
//         let random_block = generate_random_block(v.last().unwrap());
//         let mut _peer_receiver = test_msg_sender.send(Message::Blocks(vec![random_block.clone()]));
//         let reply = server_receiver.recv().unwrap();
//         if let Message::NewBlockHashes(v) = reply {
//             assert_eq!(v, vec![random_block.hash()]);
//         } else {
//             panic!();
//         }
//     }
// }

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
