pub use serde::{Deserialize, Serialize};
pub use std::{collections::HashMap, error::Error, io::Cursor};
pub use wasm_bindgen::prelude::*;
pub use pivx_primitives::consensus::{BlockHeight, MAIN_NETWORK, TEST_NETWORK};
pub use pivx_primitives::merkle_tree::{CommitmentTree, IncrementalWitness, MerklePath};
pub use pivx_primitives::sapling::{note::Note, Node, Nullifier};
pub use pivx_primitives::transaction::Transaction;
pub use pivx_primitives::zip32::AccountId;
pub use pivx_client_backend::decrypt_transaction;
pub use pivx_client_backend::keys::UnifiedFullViewingKey;
pub use pivx_primitives::zip32::Scope;
pub use crate::keys::decode_extsk;

mod test;

#[derive(Serialize, Deserialize)]
pub struct JSTxSaplingData {
    pub decrypted_notes: Vec<(Note, String)>,
    pub nullifiers: Vec<String>,
    pub commitment_tree: String,
}

//Input a tx and return: the updated commitment merkletree, all the nullifier found in the tx and all the node decoded with the corresponding witness
#[wasm_bindgen]
pub fn handle_transaction(
    tree_hex: String,
    tx: String,
    enc_extsk: String,
    is_testnet: bool,
) -> JsValue {
    let buff =
        Cursor::new(hex::decode(tree_hex).expect("Cannot decode commitment tree from hexadecimal"));
    let mut tree = CommitmentTree::<Node>::read(buff).expect("Cannot decode commitment tree!");
    let extsk = decode_extsk(&enc_extsk, is_testnet);
    let key = UnifiedFullViewingKey::new(Some(extsk.to_diversifiable_full_viewing_key()), None)
        .expect("Failed to create unified full viewing key");
    let (nullifiers, comp_note) =
        handle_transaction_internal(&mut tree, &tx, &key, true).expect("Cannot decode tx");
    let mut ser_comp_note: Vec<(Note, String)> = vec![];
    let mut ser_nullifiers: Vec<String> = vec![];
    for (note, witness) in comp_note.iter() {
        let mut buff = Vec::new();
        witness
            .write(&mut buff)
            .expect("Cannot write witness to buffer");
        ser_comp_note.push((note.clone(), hex::encode(&buff)));
    }

    for nullif in nullifiers.iter() {
        ser_nullifiers.push(hex::encode(nullif.0));
    }

    let mut buff = Vec::new();
    tree.write(&mut buff).expect("Cannot write tree to buffer");

    let res: JSTxSaplingData = JSTxSaplingData {
        decrypted_notes: ser_comp_note,
        nullifiers: ser_nullifiers,
        commitment_tree: hex::encode(buff),
    };
    serde_wasm_bindgen::to_value(&res).expect("Cannot serialize tx output")
}

//add a tx to a given commitment tree and the return a witness to each output
pub fn handle_transaction_internal(
    tree: &mut CommitmentTree<Node>,
    tx: &str,
    key: &UnifiedFullViewingKey,
    is_testnet: bool,
) -> Result<(Vec<Nullifier>, Vec<(Note, IncrementalWitness<Node>)>), Box<dyn Error>> {
    let tx = Transaction::read(
        Cursor::new(hex::decode(tx)?),
        pivx_primitives::consensus::BranchId::Sapling,
    )?;
    let mut hash = HashMap::new();
    hash.insert(AccountId::default(), key.clone());
    let decrypted_tx = if is_testnet {
        decrypt_transaction(&TEST_NETWORK, BlockHeight::from_u32(320), &tx, &hash)
    } else {
        decrypt_transaction(&MAIN_NETWORK, BlockHeight::from_u32(320), &tx, &hash)
    };
    let mut witnesses = vec![];
    let mut nullifiers: Vec<Nullifier> = vec![];
    if let Some(sapling) = tx.sapling_bundle() {
        for x in sapling.shielded_spends() {
            nullifiers.push(*x.nullifier());
        }

        for (i, out) in sapling.shielded_outputs().iter().enumerate() {
            println!("note found!");
            tree.append(Node::from_cmu(out.cmu()))
                .map_err(|_| "Failed to add cmu to tree")?;
            for note in &decrypted_tx {
                if note.index == i {
                    // Save witness
                    let witness = IncrementalWitness::from_tree(tree);
                    witnesses.push((note.note.clone(), witness));
                }
            }
        }
    }
    Ok((nullifiers, witnesses))
}

#[wasm_bindgen]
pub fn remove_spent_notes(
    notes_data: JsValue,
    nullifiers_data: JsValue,
    enc_extsk: String,
    is_testnet: bool,
) -> JsValue {
    let hex_notes: Vec<(Note, String)> =
        serde_wasm_bindgen::from_value(notes_data).expect("Cannot deserialize notes");
    let nullifiers: Vec<String> =
        serde_wasm_bindgen::from_value(nullifiers_data).expect("Cannot deserialize nullifiers");
    let mut notes: Vec<(Note, String, MerklePath<Node>)> = vec![];
    let mut unspent_notes: Vec<(Note, String)> = vec![];

    let extsk = decode_extsk(&enc_extsk, is_testnet);
    let nullif_key = extsk
        .to_diversifiable_full_viewing_key()
        .to_nk(Scope::External);

    for (note, witness) in hex_notes.iter() {
        let buff = Cursor::new(hex::decode(witness).expect("Cannot decode witness"));
        let path = IncrementalWitness::<Node>::read(buff)
            .expect("Cannot read witness from buffer")
            .path()
            .expect("Cannot find witness path");
        notes.push((note.clone(), witness.clone(), path));
    }
    for (note, witness, path) in notes.iter() {
        let nf = hex::encode(note.nf(&nullif_key, path.position).0);
        if nullifiers.iter().any(|x| **x == nf) {
            unspent_notes.push((note.clone(), witness.clone()));
        };
    }
    serde_wasm_bindgen::to_value(&unspent_notes).expect("Cannot serialize unspent notes")
}
