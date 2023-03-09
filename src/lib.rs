mod checkpoint;
mod utils;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use std::path::Path;
use std::process::Command;
use std::{collections::HashMap, error::Error, io::Cursor};

use pivx_primitives::consensus::{
    BlockHeight, MainNetwork, Parameters, TestNetwork, MAIN_NETWORK, TEST_NETWORK,
};
use pivx_primitives::merkle_tree::{CommitmentTree, IncrementalWitness, MerklePath};
use pivx_primitives::sapling::{note::Note, Node};
use pivx_primitives::transaction::Transaction;
use pivx_primitives::zip32::sapling::ExtendedSpendingKey;
use pivx_primitives::zip32::AccountId;
use pivx_primitives::zip32::DiversifierIndex;

use pivx_client_backend::decrypt_transaction;
use pivx_client_backend::encoding;
use pivx_client_backend::keys::{sapling, UnifiedFullViewingKey}; //::{decode_extended_spending_key, decode_transparent_address};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

//Data needed to generate an extended spending key
#[derive(Serialize, Deserialize)]
pub struct JSExtendedSpendingKeySerData {
    pub seed: [u8; 32],
    pub coin_type: u32,
    pub account_index: u32,
}

//Generate an extended spending key given a seed, coin type and account index
#[wasm_bindgen]
pub fn generate_extended_spending_key_from_seed(val: JsValue) -> JsValue {
    let data_arr: JSExtendedSpendingKeySerData = serde_wasm_bindgen::from_value(val)
        .expect("Cannot deserialize seed/coin type/ account index");
    let extsk = sapling::spending_key(
        &data_arr.seed,
        data_arr.coin_type,
        AccountId::from(data_arr.account_index),
    );
    let enc_str: &str = if data_arr.coin_type == 1 {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };
    return serde_wasm_bindgen::to_value(&encoding::encode_extended_spending_key(enc_str, &extsk))
        .expect("Cannot serialize extended spending key");
}

//Generate the n_address-th valid payment address given the encoded extended full viewing key and a starting index
#[wasm_bindgen]
pub fn generate_next_shielding_payment_address(
    val: JsValue,
    n_address: i32,
    isTestnet: bool,
) -> JsValue {
    let enc_extsk: String = serde_wasm_bindgen::from_value(val)
        .expect("Cannot deserialize the encoded extended full viewing key");
    let enc_str: &str = if isTestnet {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };

    let extsk = encoding::decode_extended_spending_key(&enc_str, &enc_extsk)
        .expect("Cannot decode the extended spending key");
    let mut found_addresses = 0;
    let mut diversifier_index = DiversifierIndex::new();
    loop {
        let payment_address = extsk
            .to_extended_full_viewing_key()
            .find_address(diversifier_index);
        if let Some(payment_address) = payment_address {
            found_addresses += 1;
            if found_addresses == n_address {
                let enc_str: &str = if isTestnet {
                    TEST_NETWORK.hrp_sapling_payment_address()
                } else {
                    MAIN_NETWORK.hrp_sapling_payment_address()
                };
                return serde_wasm_bindgen::to_value(&encoding::encode_payment_address(
                    &enc_str,
                    &payment_address.1,
                ))
                .expect("Cannot encode payment address");
            }
        }
        diversifier_index.increment();
    }
}

fn handle_block(
    tree: &mut CommitmentTree<Node>,
    block_data: String,
    key: &UnifiedFullViewingKey,
    isTestnet: bool,
) -> Vec<(Note, MerklePath<Node>)> {
    let block_json: serde_json::Value = serde_json::from_str(block_data.trim()).unwrap();
    let mut notes = vec![];
    for tx in block_json.get("tx").unwrap().as_array().unwrap() {
        let hex = tx.get("hex").unwrap().as_str().unwrap();
        notes.append(&mut add_tx_to_tree(tree, hex, &key, isTestnet).unwrap());
    }
    return notes;
}

//add a tx to a given commitment tree and the return a witness to each output TODO: add a witness to each input as well
fn add_tx_to_tree(
    tree: &mut CommitmentTree<Node>,
    tx: &str,
    key: &UnifiedFullViewingKey,
    isTestnet: bool,
) -> Result<Vec<(Note, MerklePath<Node>)>, Box<dyn Error>> {
    let tx = Transaction::read(
        Cursor::new(hex::decode(tx)?),
        pivx_primitives::consensus::BranchId::Sapling,
    )?;
    let mut hash = HashMap::new();
    hash.insert(AccountId::default(), key.clone());
    let decrypted_tx = if isTestnet {
        decrypt_transaction(&TEST_NETWORK, BlockHeight::from_u32(320), &tx, &hash)
    } else {
        decrypt_transaction(&MAIN_NETWORK, BlockHeight::from_u32(320), &tx, &hash)
    };
    let mut witnesses = vec![];

    if let Some(sapling) = tx.sapling_bundle() {
        for (i, out) in sapling.shielded_outputs().iter().enumerate() {
            println!("note found!");
            tree.append(Node::from_cmu(out.cmu()))
                .map_err(|_| "Failed to add cmu to tree")?;
            for note in &decrypted_tx {
                if note.index == i {
                    // Save witness
                    let witness = IncrementalWitness::from_tree(tree)
                        .path()
                        .expect("Note not found??");
                    witnesses.push((note.note.clone(), witness));
                }
            }
        }
    }
    Ok(witnesses)
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, pivx-shielding!");
}
