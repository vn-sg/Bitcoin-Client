use ring::signature::KeyPair;

use crate::types::block::{Block, Header, Content, generate_random_block_my};
use crate::types::hash::{H256, Hashable};
use crate::types::key_pair;
use crate::types::transaction::{SignedTransaction, Transaction};
use std::collections::HashMap;

pub struct Blockchain {
    pub blocks: HashMap<H256, (Block, u32)>,
    tip: H256,
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let mut block_map = HashMap::new();
        // let block: Block = generate_random_block_my(&parent);
        // Generate the genesis
        // let st_nodes = vec![
        //     SignedTransaction{
        //     transaction: Transaction::random(), 
        //     signature_vector: Vec::new(), 
        //     public_key_vector: key_pair::random().public_key().as_ref().to_vec()
        //     }, 
        //     SignedTransaction{
        //         transaction: Transaction::random(), 
        //         signature_vector: Vec::new(), 
        //         public_key_vector: key_pair::random().public_key().as_ref().to_vec()
        //         }, 
        //     SignedTransaction{
        //         transaction: Transaction::random(), 
        //         signature_vector: Vec::new(), 
        //         public_key_vector: key_pair::random().public_key().as_ref().to_vec()
        //         }, 
        // ];
        let genesis = Block {
            head: Header{
                parent: [0u8; 32].into(),
                nonce: 0,
                difficulty: hex_literal::hex!("000ff93a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into(),
                timestamp: 0,
                merkle_root: [0u8; 32].into(),
            },
            cont: Content{
                st: Vec::new()
            }
        };
        let genesis_hash: H256 = genesis.hash();
        block_map.insert(genesis_hash, (genesis, 0));
        Self {
            blocks: block_map,
            tip: genesis_hash
        }
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) {
        let height: u32 = self.blocks.get(&block.head.parent).unwrap().1 + 1;
        let prev_tip_height: u32 = self.blocks.get(&self.tip).unwrap().1;
        let block_hash: H256 = block.hash();
        self.blocks.insert(block_hash, (block.clone(), height));
        if height > prev_tip_height{
            self.tip = block_hash;
        }
        
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.tip
    }

    /// Get all blocks' hashes of the longest chain, ordered from genesis to the tip
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut hashes: Vec<H256> =  Vec::new();
        let (block, mut height) = self.blocks.get(&self.tip).unwrap();
        hashes.push(self.tip);
        let mut next_hash = block.head.parent;
        while height >= 1 {
            hashes.push(next_hash);
            next_hash = self.blocks.get(&next_hash).unwrap().0.head.parent;
            height -= 1;
        }

        hashes.reverse();
        hashes
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());

    }

    #[test]
    fn insert1000() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let mut block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        let mut block_map: HashMap<H256, u32> = HashMap::new();
        block_map.insert(genesis_hash, 0);
        block_map.insert(block.hash(), 1);

        // println!("{:?}",(blockchain.blocks.iter())[0]);
        let mut tip_h = 0;
        let mut tip: H256 = genesis_hash;
        let mut parent_hash: H256;
        let mut h: u32 = 0;
        let mut random;
        for _ in 0..1000 {
            random = blockchain.blocks.iter().next().unwrap();
            parent_hash = *random.0;
            block = generate_random_block(&parent_hash);
            
            blockchain.insert(&block);
            h = block_map.get(&parent_hash).unwrap() + 1;
            // b.bm.insert(block.hash(), h);
            block_map.insert(block.hash(), h);
            
            if h > tip_h {
                tip_h = h;
                tip = block.hash();
            }
        }

        assert_eq!(blockchain.tip(), tip);
        
    }


}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST