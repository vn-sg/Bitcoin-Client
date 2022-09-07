use std::collections::HashMap;

use serde::{Serialize,Deserialize};
use ring::signature::{Ed25519KeyPair, Signature, KeyPair, VerificationAlgorithm, EdDSAParameters};
use rand::{thread_rng, Rng};
use crate::types::hash::H256;
use crate::types::address::Address;
use hex_literal::hex;

// #[derive(Eq, PartialEq, Serialize, Deserialize, Clone, Hash, Default, Copy, Debug)]
// pub struct Address([u8; 20]);

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    // sender: Address, 
    // receiver: Address,
    // value: i32,
    pub input: Vec<Input>,
    pub output: Vec<Output>,
}

impl Transaction {
    pub fn random() -> Self {
        Self {
            input: vec![Input::random()],
            output: vec![Output::random()],
        }
    }
    // pub fn pass_check(hash: &H256, index: &u8, balance: &u32, peer_addrs: &(Address, Address)) -> Self {
        pub fn pass_check(inputs: &Vec<Input>, balance: &u32, peer_addrs: &(Address, Address)) -> Self {
        Self {
            input: inputs.to_vec(),
            output: Output::pass_check(balance, peer_addrs),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Input {
    pub prev_trans: H256,
    pub index: u8,
}
impl Input{
    pub fn random() -> Self {
        Self {
            prev_trans: [rand::random(); 32].into(),
            index: rand::thread_rng().gen(),
        }
    }
    pub fn pass_check(hash: &H256, ind: &u8) -> Self {
        Self {
            prev_trans: *hash,
            index: *ind,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Output {
    pub recipient_addr: Address,
    pub value: u32,
}
impl Output{
    pub fn random() -> Self {
        Self {
            recipient_addr: Address([rand::random(); 20]),
            value: rand::thread_rng().gen(),
        }
    }
    pub fn pass_check(balance: &u32, peer_addrs: &(Address, Address)) -> Vec<Self> {
        // let v1: u32 = rand::random::<u32>() % balance;
        let v1: u32 = balance/3;
        let v2: u32 = (balance - v1)/2;
        let v3: u32 = balance - v1 - v2;
        // let mut outputs: Vec<Self> = Vec::new();

        if v1 == 0 || v2 == 0 || v3 == 0 {
            vec![
                Self {
                recipient_addr: peer_addrs.0,
                value: *balance
                }
            ]
        }
        else {
            vec![
                Self {
                    recipient_addr: peer_addrs.0,
                    value: v1,
                },
                Self {
                    recipient_addr: peer_addrs.1,
                    value: v2,
                },
                Self {
                    recipient_addr: peer_addrs.1,
                    value: v3,
                },

            ]
        }

    }
}

pub struct Mempool {
    pub trans: HashMap<H256,SignedTransaction>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTransaction {
    pub transaction: Transaction,
    pub signature_vector: Vec<u8>,
    pub public_key_vector: Vec<u8>,

}
// 
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct State {
    pub states: HashMap<(H256, u8), (u32, Address)>,
}

impl State {

    pub fn new(pubic_key1: &Ed25519KeyPair) -> Self{
        // Initialize a new HashMap which is going to be used for the State Struct
        let mut s: HashMap<(H256, u8), (u32, Address)> = HashMap::new();
        
        // Do initial coin offering (ICO) by inserting an entry into state
        let tx_hash: H256 = hex!("0000000000000000000000000000000000000000000000000000000000000000").into();
        s.insert((tx_hash, 0), (100 as u32, Address::from_public_key_bytes(pubic_key1.public_key().as_ref())));
        // let to_insert =  vec![
        //     ((tx_hash, 0), (10 as u32, Address::from_public_key_bytes(pubic_keys[0].public_key().as_ref()))),
        //     ((tx_hash, 0), (0 as u32, Address::from_public_key_bytes(pubic_keys[1].public_key().as_ref()))),
        //     ((tx_hash, 0), (0 as u32, Address::from_public_key_bytes(pubic_keys[2].public_key().as_ref())))
        //     ];
        // to_insert.into_iter().map(|(k,v) |s.insert(k,v));
        State {
            states: s
        }
    }

    pub fn update(&mut self, transaction: &SignedTransaction) {
        let input = transaction.transaction.clone().input;
        let output = transaction.transaction.clone().output;
        // let tx_hash = transaction.hash();
        let tx_hash = ring::digest::digest(&ring::digest::SHA256, &bincode::serialize(transaction).unwrap()).into();
        for i in input {
            let to_find = (i.prev_trans, i.index);
            if self.states.contains_key(&to_find) {
                self.states.remove(&to_find);
            }
        }

        for o in 0..output.len() {
            self.states.insert((tx_hash, o as u8), (output[o].value, output[o].recipient_addr));
        }
    }
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            trans: HashMap::new()
        }
    }
}
/*
impl Address {
    pub fn from_public_key_bytes(bytes: &[u8]) -> Address {
        let c = ring::digest::digest(&ring::digest::SHA256, bytes);
        use std::convert::TryInto;

        fn slice_array(b: &[u8]) -> [u8; 20] {
            let sliced :&[u8] = &(b[(b.len() - 20) .. b.len()]);
            return sliced.try_into().expect("Length is Incorrect");
        }
        Address(slice_array(c.as_ref()))
    }
}
*/

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    let rng_gen = ring::rand::SystemRandom::new();
 
    // const MESSAGE: &[u8] = b"hello, world";
    let t_byte = bincode::serialize(t).unwrap();
    let sig = key.sign(&t_byte);

    return sig;
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    let peer_public_key = ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public_key);
    // const MESSAGE: &[u8] = b"hello, world";
    let t_byte = bincode::serialize(t).unwrap();
    let result = peer_public_key.verify(&t_byte, signature);
    match result {
        Ok(b) => return true,
        Err(error) => return false,
    };

}

// #[cfg(any(test, test_utilities))]
// pub fn generate_random_transaction() -> Transaction {
//     let mut rng = rand::thread_rng();
//     let rand_sender : &[u8] = &bincode::serialize(&rng.gen::<i32>()).unwrap();
//     let sender_addr = Address::from_public_key_bytes(&rand_sender);
//     let rand_receiver : &[u8] = &bincode::serialize(&rng.gen::<i32>()).unwrap();
//     let receiver_addr = Address::from_public_key_bytes(&rand_receiver);

//     let rand_value : i32 = rng.gen();

//     Transaction{
//         sender: sender_addr, 
//         receiver: sender_addr,
//         value: rand_value,
//     }
// }

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::types::key_pair;
//     use ring::signature::KeyPair;


//     #[test]
//     fn sign_verify() {
//         let t = generate_random_transaction();
//         let key = key_pair::random();
//         let signature = sign(&t, &key);
//         assert!(verify(&t, key.public_key().as_ref(), signature.as_ref()));
//     }
//     #[test]
//     fn sign_verify_two() {
//         let t = generate_random_transaction();
//         let key = key_pair::random();
//         let signature = sign(&t, &key);
//         let key_2 = key_pair::random();
//         let t_2 = generate_random_transaction();
//         assert!(!verify(&t_2, key.public_key().as_ref(), signature.as_ref()));
//         assert!(!verify(&t, key_2.public_key().as_ref(), signature.as_ref()));
//     }
// }

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST