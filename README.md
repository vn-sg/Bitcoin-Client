# Bitcoin Client


## Introduction

This project is built to better understand the fundamental infrastructure of the actual Cryptocurrency Client System of Bitcoin. Though being simplified, it contains all critical components and remains fully operational with the well-known longest-chain protocol. 

This Bitcoin client is implemented with Rust, with the support of mining, adjustment of difficulty, proof of work
(PoW) validation, Merkle proof, transactions, and network module that formed p2p networks and used the gossip
protocol to exchange data.
<br><br>

## Development Specifications

### struct ***Block***

* **`parent`** - a hash pointer to parent block. Represented as *`H256`*.

* **`nonce`** - a random integer that will be used in proof-of-work mining. `u32` is straightforward option.
* **`difficulty`** - the mining difficulty, i.e., the threshold in proof-of-work check. Implemented with type *`H256`* to take advange of the comparison function, with which hash can be directly compared with difficulty. (i.e. `hash <= difficulty`)
* **`timestamp`** - the timestamp when this block is generated.
* **`merkle_root`** - the Merkle root of data (explained later).

The above are commonly known as Header. These fields are included in the struct `Header`.

* **`data/content`** - the actual transactions carried by this block. Represented as Vectors (`Vec`) of SignedTransaction.

Content is included in struct `Content`.
<br><br>

### struct ***Blockchain***

struct `Blockchain`, which contains the necessary information of a direct acyclic graph (DAG) and provides functions related to the longest chain rule. The following functions are required:

* **`new()`** - create a new blockchain that only contains the information of the genesis block. (Genesis block should be pre-defined)
* **`insert()`** - insert a block into the blockchain. 
* **`tip()`** - return the last block's hash in the longest chain.
* **`all_blocks_in_longest_chain()`** - return all blocks' hashes in a vector, from the genesis to the tip.


## Feature Examples

### PoW validity check
Check if:

PoW check: check if block.hash() <= difficulty. (Note that difficulty is a misnomer here since a higher 'difficulty' here means that the block is easier to mine).
Difficulty in the block header is consistent with your view. We have a fixed mining difficulty for this project, thus, this would just involve checking if difficulty equals the parent block's difficulty. (This step should be done after parent check.)


### Parent check

Check if the block's parent exists in the blockchain, if the parent exists, add the block to the blockchain.
If this check fails, the block shall need to be inserted into an *`orphan buffer`*. 

If check fails, also construct and push GetBlocks message, containing this parent hash.

### Orphan block handler
Check if the new processed block is a parent to any block in the orphan buffer, if that is the case, remove the block from *`orphan buffer`* and process the block. This step should be done iteratively. I.e., a block makes a former orphan block be processed, and the latter makes another former orphan block be processed, and so on.