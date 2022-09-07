use serde::{Serialize, Deserialize};
use crate::types::hash::{H256, Hashable};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use super::transaction::SignedTransaction;
use super::merkle::MerkleTree;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub head: Header,
    pub cont: Content,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub parent: H256,
    pub nonce: u32,
    pub difficulty: H256,
    pub timestamp: u128,
    pub merkle_root:H256,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Content{
    pub st: Vec<SignedTransaction>,
}

impl Hashable for SignedTransaction {
    fn hash(&self) -> H256 {
        ring::digest::digest(&ring::digest::SHA256, &bincode::serialize(self).unwrap()).into()
    }
}

impl Hashable for Header {
    fn hash(&self) -> H256 {
        ring::digest::digest(&ring::digest::SHA256, &bincode::serialize(self).unwrap()).into()
    }
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        let Block{head: h, cont: _} = self;
        h.hash()
    }
}

impl Block {
    pub fn get_parent(&self) -> H256 {
        self.head.parent
    }

    pub fn get_difficulty(&self) -> H256 {
        self.head.difficulty
    }
}

pub fn generate_random_block_my(parent: &H256) -> Block {
    use rand::Rng;
    let empty_l: [H256; 0] = [];
    Block {
        head: Header{
            parent: *parent,
            nonce: rand::thread_rng().gen(),
            difficulty: hex_literal::hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into(),
            timestamp: match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(n) => n.as_millis(),
                Err(_) => panic!("SystemTime before UNIX EPOCH!"),
            },
            merkle_root: MerkleTree::new(&empty_l).root(),
        },
        cont: Content{
            st: Vec::new()
        }
    }
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_block(parent: &H256) -> Block {
    use rand::Rng;
    let empty_l: [H256; 0] = [];
    Block {
        head: Header{
            parent: *parent,
            nonce: rand::thread_rng().gen(),
            difficulty: hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into(),
            timestamp: match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(n) => n.as_millis(),
                Err(_) => panic!("SystemTime before UNIX EPOCH!"),
            },
            merkle_root: MerkleTree::new(&empty_l).root(),
        },
        cont: Content{
            st: Vec::new()
        }
    }
}