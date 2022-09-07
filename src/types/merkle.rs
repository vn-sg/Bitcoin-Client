use super::hash::{Hashable, H256};

/// A Merkle tree.
#[derive(Debug, Default)]
pub struct MerkleTree {
    treeList: Vec<Vec<H256>>,
}

impl MerkleTree {
    pub fn new<T>(data: &[T]) -> Self where T: Hashable, {
        if data.is_empty() {
            return MerkleTree{
                treeList: Vec::new(),
            }
        }

        // Define level0 to be the leaf level
        let mut level0= Vec::new();

        // Put the hash of each data into the leaf level of the tree
        for d in data.iter(){
            level0.push(d.hash());
        }
        // Define the tree vector
        let mut tree = Vec::new();
        if  level0.len() != 1 && level0.len() % 2 != 0 {
            level0.push(level0[level0.len()-1]);
        }
        // push leaf level as level 0
        tree.push(level0);
        
        let mut current_level : usize = 0;
        loop {
            if current_level >= tree.len() {
                break;
            }
            if tree[current_level].len() == 1 {
                break;
            }
            let mut index : usize = 0;
            let mut next_level = Vec::new();
            loop {
                if index >= tree[current_level].len() {
                    break;
                }
                let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
                hash.update(tree[current_level][index].as_ref());
                hash.update(tree[current_level][index+1].as_ref());
                let combined : H256 = hash.finish().into();
                next_level.push(combined);
                // std::mem::replace(&mut tree[current_level][index], to_replace);
                // tree[current_level].remove(index+1);
                
                index += 2;
            }
            if  next_level.len() != 1 && next_level.len() % 2 != 0 {
                next_level.push(next_level[next_level.len()-1]);
            }
            tree.push(next_level);
            current_level += 1;
        }

        // let mut p = 0u32;
        // let mut last_index; 
        // let mut size = level0.len();
        // loop {
        //     if size <= 1 {
        //         break;
        //     }
        //     if size % 2 != 0 {
        //         last_index = level0.len()-1;
        //         for index in (last_index+1-usize::pow(2,p))..last_index {
        //             level0.push(level0[index]);
        //         }
        //     }
        //     p += 1;
        //     size /= 2;
        // }

        MerkleTree{
            treeList: tree,
        }

    }

    pub fn root(&self) -> H256 {
        // let mut m_tree = self.treeList.clone();
        // loop {
        //     if m_tree.len() == 1 {
        //         break;
        //     }
        //     let mut index = 0;
        //     loop {
        //         if index >= m_tree.len() {
        //             break;
        //         }
        //         let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        //         hash.update(m_tree[index].as_ref());
        //         hash.update(m_tree[index+1].as_ref());
        //         let to_replace : H256 = hash.finish().into();
        //         std::mem::replace(&mut m_tree[index], to_replace);
        //         m_tree.remove(index+1);
                
        //         index += 2;
        //     }
        // }
        if self.treeList.is_empty() {
            return (hex_literal::hex!("0000000000000000000000000000000000000000000000000000000000000000")).into()
        }
        let tree = &self.treeList;
        *tree.get(tree.len()-1).unwrap().get(0).unwrap()
        // {
                // Some(x) => *x,
                // None    => {
                //     let r : [u8; 32] = hex_literal::hex!("00");
                //     "0".into()
                // },
        // }

    }

    /// Returns the Merkle Proof of data at index i
    pub fn proof(&self, index: usize) -> Vec<H256> {
        let mut proof_list = Vec::new();
        if index < 0 as usize || index >= self.treeList[0].len() {
            return proof_list;
        }
        let mut i = index;
        let mut level: usize = 0;
        loop {
            if level >= self.treeList.len()-1 {
                return proof_list;
            }
            if i % 2 == 0 {
                proof_list.push(self.treeList[level][i+1]);
            }
            else {
                proof_list.push(self.treeList[level][i-1]);
            }

            i /= 2;
            level += 1;
        }

    }
}

/// Verify that the datum hash with a vector of proofs will produce the Merkle root. Also need the
/// index of datum and `leaf_size`, the total number of leaves.
pub fn verify(root: &H256, datum: &H256, proof: &[H256], index: usize, leaf_size: usize) -> bool {
    if index < 0 as usize || index >= leaf_size {
        return false;
    } 
    if leaf_size <= 0 {
        return false;
    }
    // if leaf_size & (leaf_size-1) != 0 {
    //     return false;
    // }
    let mut current_hash  = *datum;
    let mut proof_index: usize = 0;
    let mut datum_index = index;
    loop {
        if proof_index >= proof.len() {
            break;
        }

        let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
        if datum_index % 2 == 0 {
            hash.update(current_hash.as_ref());
            hash.update(proof[proof_index].as_ref());
        }
        else {
            hash.update(proof[proof_index].as_ref());
            hash.update(current_hash.as_ref());
        }

        current_hash = hash.finish().into();

        proof_index += 1;
        datum_index /= 2;

    }

    current_hash == *root

        
}
// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use crate::types::hash::H256;
    use super::*;

    macro_rules! gen_merkle_tree_data {
        () => {{
            vec![
                (hex!("0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d")).into(),
                (hex!("0101010101010101010101010101010101010101010101010101010101010202")).into(),
            ]
        }};
    }

    #[test]
    fn merkle_root() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let root = merkle_tree.root();
        assert_eq!(
            root,
            (hex!("6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920")).into()
        );
        // "b69566be6e1720872f73651d1851a0eae0060a132cf0f64a0ffaea248de6cba0" is the hash of
        // "0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d0a0b0c0d0e0f0e0d"
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
        // "6b787718210e0b3b608814e04e61fde06d0df794319a12162f287412df3ec920" is the hash of
        // the concatenation of these two hashes "b69..." and "965..."
        // notice that the order of these two matters
    }

    #[test]
    fn merkle_proof() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert_eq!(proof,
                   vec![hex!("965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f").into()]
        );
        // "965b093a75a75895a351786dd7a188515173f6928a8af8c9baa4dcff268a4f0f" is the hash of
        // "0101010101010101010101010101010101010101010101010101010101010202"
    }

    #[test]
    fn merkle_verifying() {
        let input_data: Vec<H256> = gen_merkle_tree_data!();
        let merkle_tree = MerkleTree::new(&input_data);
        let proof = merkle_tree.proof(0);
        assert!(verify(&merkle_tree.root(), &input_data[0].hash(), &proof, 0, input_data.len()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST