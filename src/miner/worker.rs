use crossbeam::channel::{unbounded, Receiver, Sender, TryRecvError};
use log::{debug, info};
use crate::network::message::Message;
use crate::types::address::Address;
use crate::types::block::Block;
use crate::network::server::Handle as ServerHandle;
use crate::types::hash::{Hashable, H256};
use std::thread;
use std::sync::{Arc, Mutex};
use crate::blockchain::Blockchain;
use std::collections::{HashMap};
use crate::types::transaction::{State, SignedTransaction, verify};

#[derive(Clone)]
pub struct Worker {
    server: ServerHandle,
    finished_block_chan: Receiver<Block>,
    blockchain: Arc<Mutex<Blockchain>>,
    block_state: Arc<Mutex<HashMap<H256, State>>>, 
}

impl Worker {
    pub fn new(
        server: &ServerHandle,
        finished_block_chan: Receiver<Block>,
        bc: &Arc<Mutex<Blockchain>>,
        block_state: &Arc<Mutex<HashMap<H256, State>>>,
    ) -> Self {
        Self {
            server: server.clone(),
            finished_block_chan,
            blockchain: Arc::clone(bc),
            block_state: Arc::clone(block_state),
        }
    }

    pub fn start(self) {
        thread::Builder::new()
            .name("miner-worker".to_string())
            .spawn(move || {
                self.worker_loop();
            })
            .unwrap();
        info!("Miner initialized into paused mode");
    }

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
        loop {
            let _block = self.finished_block_chan.recv().expect("Receive finished block error");

            // let mut valid = true;
            // let mut cur_state = self.block_state.lock().unwrap()[&_block.head.parent].clone();
            // let block_trans = _block.clone().cont.st;
            // for tx in block_trans {
            //     let signed_trans = tx.clone();
            //     let trans = signed_trans.transaction;
            //     let pubkey = signed_trans.public_key_vector;
            //     let key_hash : H256 = ring::digest::digest(&ring::digest::SHA256, pubkey.as_ref()).into();
            //     let input = trans.input;
            //     let output = trans.output;
            //     let mut input_value = 0;
            //     for each_in in input {
            //         let prev = each_in.prev_trans;
            //         let prev_index = each_in.index;
            //         if cur_state.states.contains_key(&(prev, prev_index)) {
            //             input_value += cur_state.states[&(prev, prev_index)].0;
            //             let recipient:Address = cur_state.states[&(prev, prev_index)].1;
            //             let _recipient:Address = Address::from_public_key_bytes(key_hash.as_ref());
            //             if recipient != _recipient {
            //                 valid = false;
            //                 break;
            //             }
            //         }
            //         else {
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

            // // block state update
            // for tx in _block.clone().cont.st {
            //     cur_state.update(&tx);
            // }

            // if valid {
            // self.block_state.lock().unwrap().insert(_block.hash(), cur_state.clone());
            // self.blockchain.lock().unwrap().insert(&_block); 

            // let block_hash: Vec<H256> = vec![_block.hash()];
            // self.server.broadcast(Message::NewBlockHashes(block_hash));
            // }

            // TODO for student: insert this finished block to blockchain, and broadcast this block hash

            if self.check_tx_state(&_block.head.parent, &_block.cont.st, &_block.hash()) {
                self.blockchain.lock().unwrap().insert(&_block);
                let block_hash: Vec<H256> = vec![_block.hash()];
                self.server.broadcast(Message::NewBlockHashes(block_hash));
            }
             

            
            
        }
    }
}
