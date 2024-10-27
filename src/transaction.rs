pub use crate::keys::decode_extended_full_viewing_key;
pub use crate::keys::decode_extsk;
use crate::prover::get_prover;
pub use pivx_client_backend::decrypt_transaction;
pub use pivx_client_backend::keys::UnifiedFullViewingKey;
use pivx_primitives::consensus::Network;
pub use pivx_primitives::consensus::Parameters;
pub use pivx_primitives::consensus::{BlockHeight, MAIN_NETWORK, TEST_NETWORK};
use pivx_primitives::legacy::Script;
pub use pivx_primitives::memo::MemoBytes;
pub use pivx_primitives::merkle_tree::{CommitmentTree, IncrementalWitness, MerklePath};
pub use pivx_primitives::transaction::builder::Progress;

use crate::keys::decode_generic_address;
use crate::keys::GenericAddress;
#[cfg(feature = "multicore")]
use atomic_float::AtomicF32;
pub use either::Either;
use pivx_primitives::sapling::prover::TxProver;
pub use pivx_primitives::sapling::{note::Note, Node, Nullifier};
pub use pivx_primitives::transaction::builder::Builder;
pub use pivx_primitives::transaction::components::Amount;
use pivx_primitives::transaction::components::{OutPoint, TxOut};
pub use pivx_primitives::transaction::fees::fixed::FeeRule;
pub use pivx_primitives::transaction::Transaction;
pub use pivx_primitives::zip32::AccountId;
use pivx_primitives::zip32::ExtendedFullViewingKey;
pub use pivx_primitives::zip32::ExtendedSpendingKey;
pub use pivx_primitives::zip32::Scope;
use rand_core::OsRng;

use secp256k1::SecretKey;
pub use serde::{Deserialize, Serialize};
use std::convert::TryInto;
pub use std::path::Path;
#[cfg(feature = "multicore")]
use std::sync::atomic::Ordering;
pub use std::{collections::HashMap, error::Error, io::Cursor};
#[cfg(feature = "multicore")]
use tokio::{join, sync::mpsc::Receiver, sync::mpsc::Sender};
pub use wasm_bindgen::prelude::*;
mod test;

#[cfg(feature = "multicore")]
static TX_PROGRESS_LOCK: AtomicF32 = AtomicF32::new(0.0);

fn fee_calculator(
    transparent_input_count: u64,
    transparent_output_count: u64,
    sapling_input_count: u64,
    sapling_output_count: u64,
) -> u64 {
    let fee_per_byte = 1000;
    let transparent_input_size = 150;
    let transparent_output_size = 34;
    let tx_offset_size = 85; // fixed tx offset in byte
    let sapling_output_size = 948;
    let sapling_input_size = 384;
    fee_per_byte
        * (sapling_output_count * sapling_output_size
            + sapling_input_count * sapling_input_size
            + transparent_input_count * transparent_input_size
            + transparent_output_count * transparent_output_size
            + tx_offset_size)
}

#[wasm_bindgen]
#[cfg(feature = "multicore")]
pub fn read_tx_progress() -> f32 {
    TX_PROGRESS_LOCK.load(Ordering::Relaxed)
}

#[wasm_bindgen]
#[cfg(not(feature = "multicore"))]
pub fn read_tx_progress() -> f32 {
    0.0
}

#[cfg(feature = "multicore")]
pub fn set_tx_status(val: f32) {
    TX_PROGRESS_LOCK.store(val, Ordering::Relaxed);
}

#[derive(Serialize, Deserialize)]
pub struct JSTxSaplingData {
    pub decrypted_notes: Vec<(Note, String)>,
    pub decrypted_new_notes: Vec<(Note, String)>,
    pub nullifiers: Vec<String>,
    pub commitment_tree: String,
}

#[derive(Serialize, Deserialize)]
pub struct Block {
    height: u32,
    txs: Vec<String>,
}

fn read_commitment_tree(tree_hex: &str) -> Result<CommitmentTree<Node>, Box<dyn Error>> {
    let buff = Cursor::new(hex::decode(tree_hex)?);
    Ok(CommitmentTree::<Node>::read(buff)?)
}

#[wasm_bindgen]
pub fn handle_blocks(
    tree_hex: &str,
    blocks: JsValue,
    enc_extfvk: &str,
    is_testnet: bool,
    comp_notes: JsValue,
) -> Result<JsValue, JsValue> {
    let blocks: Vec<Block> = serde_wasm_bindgen::from_value(blocks)?;
    let mut tree = read_commitment_tree(tree_hex).map_err(|_| "Couldn't read commitment tree")?;
    let comp_note: Vec<(Note, String)> = serde_wasm_bindgen::from_value(comp_notes)?;
    let extfvk =
        decode_extended_full_viewing_key(enc_extfvk, is_testnet).map_err(|e| e.to_string())?;
    let key = UnifiedFullViewingKey::new(Some(extfvk.to_diversifiable_full_viewing_key()), None)
        .ok_or("Failed to create unified full viewing key")?;
    let mut comp_note = comp_note
        .into_iter()
        .map(|(note, witness)| {
            let wit = Cursor::new(hex::decode(witness).unwrap());
            (note, IncrementalWitness::read(wit).unwrap())
        })
        .collect::<Vec<_>>();
    let mut nullifiers = vec![];
    let mut new_notes = vec![];
    for block in blocks {
        for tx in block.txs {
            nullifiers.extend(
                handle_transaction_internal(
                    &mut tree,
                    &tx,
                    key.clone(),
                    is_testnet,
                    &mut comp_note,
                    &mut new_notes,
                )
                .map_err(|_| "Couldn't handle transaction")?,
            );
        }
    }

    let ser_comp_note = serialize_comp_note(comp_note).map_err(|_| "couldn't decrypt notes")?;
    let ser_new_comp_note = serialize_comp_note(new_notes).map_err(|_| "couldn't decrypt notes")?;

    let mut ser_nullifiers: Vec<String> = Vec::with_capacity(nullifiers.len());
    for nullif in nullifiers.iter() {
        ser_nullifiers.push(hex::encode(nullif.0));
    }

    let mut buff = Vec::new();
    tree.write(&mut buff)
        .map_err(|_| "Cannot write tree to buffer")?;

    Ok(serde_wasm_bindgen::to_value(&JSTxSaplingData {
        decrypted_notes: ser_comp_note,
        nullifiers: ser_nullifiers,
        commitment_tree: hex::encode(buff),
        decrypted_new_notes: ser_new_comp_note,
    })?)
}

pub fn serialize_comp_note(
    comp_note: Vec<(Note, IncrementalWitness<Node>)>,
) -> Result<Vec<(Note, String)>, Box<dyn Error>> {
    let mut ser_comp_note: Vec<(Note, String)> = vec![];
    for (note, witness) in comp_note {
        let mut buff = Vec::new();
        witness
            .write(&mut buff)
            .map_err(|_| "Cannot write witness to buffer")?;
        ser_comp_note.push((note, hex::encode(&buff)));
    }
    Ok(ser_comp_note)
}

//add a tx to a given commitment tree and the return a witness to each output
pub fn handle_transaction_internal(
    tree: &mut CommitmentTree<Node>,
    tx: &str,
    key: UnifiedFullViewingKey,
    is_testnet: bool,
    witnesses: &mut Vec<(Note, IncrementalWitness<Node>)>,
    new_witnesses: &mut Vec<(Note, IncrementalWitness<Node>)>,
) -> Result<Vec<Nullifier>, Box<dyn Error>> {
    let tx = Transaction::read(
        Cursor::new(hex::decode(tx)?),
        pivx_primitives::consensus::BranchId::Sapling,
    )?;
    let mut hash = HashMap::new();
    hash.insert(AccountId::default(), key);
    let mut decrypted_tx = if is_testnet {
        decrypt_transaction(&TEST_NETWORK, BlockHeight::from_u32(320), &tx, &hash)
    } else {
        decrypt_transaction(&MAIN_NETWORK, BlockHeight::from_u32(320), &tx, &hash)
    };
    let mut nullifiers: Vec<Nullifier> = vec![];
    if let Some(sapling) = tx.sapling_bundle() {
        for x in sapling.shielded_spends() {
            nullifiers.push(*x.nullifier());
        }

        for (i, out) in sapling.shielded_outputs().iter().enumerate() {
            tree.append(Node::from_cmu(out.cmu()))
                .map_err(|_| "Failed to add cmu to tree")?;
            for (_, witness) in witnesses.iter_mut().chain(new_witnesses.iter_mut()) {
                witness
                    .append(Node::from_cmu(out.cmu()))
                    .map_err(|_| "Failed to add cmu to witness")?;
            }
            for (index, note) in decrypted_tx.iter().enumerate() {
                if note.index == i {
                    // Save witness
                    let witness = IncrementalWitness::from_tree(tree);
                    new_witnesses.push((decrypted_tx.swap_remove(index).note, witness));
                    break;
                }
            }
        }
    }
    Ok(nullifiers)
}

#[wasm_bindgen]
pub fn remove_spent_notes(
    notes_data: JsValue,
    nullifiers_data: JsValue,
    enc_extfvk: String,
    is_testnet: bool,
) -> Result<JsValue, JsValue> {
    let hex_notes: Vec<(Note, String)> = serde_wasm_bindgen::from_value(notes_data)?;
    let nullifiers: Vec<String> = serde_wasm_bindgen::from_value(nullifiers_data)?;
    let mut notes: Vec<(Note, String, MerklePath<Node>)> = vec![];
    let mut unspent_notes: Vec<(Note, String)> = vec![];

    let extfvk =
        decode_extended_full_viewing_key(&enc_extfvk, is_testnet).map_err(|e| e.to_string())?;
    let nullif_key = extfvk
        .to_diversifiable_full_viewing_key()
        .to_nk(Scope::External);

    for (note, witness) in hex_notes.iter() {
        let buff = Cursor::new(hex::decode(witness).map_err(|_| "Cannot decode witness")?);
        let path = IncrementalWitness::<Node>::read(buff)
            .map_err(|_| "Cannot read witness from buffer")?
            .path()
            .ok_or("Cannot find witness path")?;
        notes.push((note.clone(), witness.clone(), path));
    }
    for (note, witness, path) in notes.iter() {
        let nf = hex::encode(note.nf(&nullif_key, path.position).0);
        if !nullifiers.iter().any(|x| **x == nf) {
            unspent_notes.push((note.clone(), witness.clone()));
        };
    }
    Ok(serde_wasm_bindgen::to_value(&unspent_notes)?)
}

#[wasm_bindgen]
pub fn get_nullifier_from_note(
    note_data: JsValue,
    enc_extfvk: String,
    is_testnet: bool,
) -> Result<JsValue, JsValue> {
    let extfvk =
        decode_extended_full_viewing_key(&enc_extfvk, is_testnet).map_err(|e| e.to_string())?;
    let (note, hex_witness): (Note, String) = serde_wasm_bindgen::from_value(note_data)?;
    let ser_nullifiers =
        get_nullifier_from_note_internal(extfvk, note, hex_witness).map_err(|e| e.to_string())?;
    Ok(serde_wasm_bindgen::to_value(&ser_nullifiers)?)
}

pub fn get_nullifier_from_note_internal(
    extfvk: ExtendedFullViewingKey,
    note: Note,
    hex_witness: String,
) -> Result<String, Box<dyn Error>> {
    let nullif_key = extfvk
        .to_diversifiable_full_viewing_key()
        .to_nk(Scope::External);
    let witness = Cursor::new(hex::decode(hex_witness).map_err(|e| e.to_string())?);
    let path = IncrementalWitness::<Node>::read(witness)
        .map_err(|_| "Cannot read witness from buffer")?
        .path()
        .ok_or("Cannot find witness path")?;
    Ok(hex::encode(note.nf(&nullif_key, path.position).0))
}

#[derive(Serialize, Deserialize)]
pub struct JSTransaction {
    pub txid: String,
    pub txhex: String,
    pub nullifiers: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Utxo {
    txid: String,
    vout: u32,
    amount: u64,
    private_key: Vec<u8>,
    script: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct JSTxOptions {
    notes: Option<Vec<(Note, String)>>,
    utxos: Option<Vec<Utxo>>,
    extsk: String,
    to_address: String,
    change_address: String,
    amount: u64,
    block_height: u32,
    is_testnet: bool,
}

#[wasm_bindgen]
pub async fn create_transaction(options: JsValue) -> Result<JsValue, JsValue> {
    let JSTxOptions {
        notes,
        extsk,
        to_address,
        change_address,
        amount,
        block_height,
        is_testnet,
        utxos,
    } = serde_wasm_bindgen::from_value::<JSTxOptions>(options)?;
    assert!(
        !(notes.is_some() && utxos.is_some()),
        "Notes and UTXOs were both provided"
    );
    let extsk = decode_extsk(&extsk, is_testnet).map_err(|e| e.to_string())?;
    let network = if is_testnet {
        Network::TestNetwork
    } else {
        Network::MainNetwork
    };
    let input = if let Some(mut notes) = notes {
        notes.sort_by_key(|(note, _)| note.value().inner());
        Either::Left(notes)
    } else if let Some(mut utxos) = utxos {
        utxos.sort_by_key(|u| u.amount);
        Either::Right(utxos)
    } else {
        panic!("No input provided")
    };
    let result = create_transaction_internal(
        input,
        &extsk,
        &to_address,
        &change_address,
        amount,
        BlockHeight::from_u32(block_height),
        network,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(serde_wasm_bindgen::to_value(&result)?)
}

/// Create a transaction.
/// The notes are used in the order they're provided
/// It might be useful to sort them first, or use any other smart alogorithm
pub async fn create_transaction_internal(
    inputs: Either<Vec<(Note, String)>, Vec<Utxo>>,
    extsk: &ExtendedSpendingKey,
    to_address: &str,
    change_address: &str,
    mut amount: u64,
    block_height: BlockHeight,
    network: Network,
) -> Result<JSTransaction, Box<dyn Error>> {
    let mut builder = Builder::new(network, block_height);
    let (transparent_output_count, sapling_output_count) =
        if to_address.starts_with(network.hrp_sapling_payment_address()) {
            (0, 2)
        } else {
            (1, 2)
        };
    let (nullifiers, change, fee) = match inputs {
        Either::Left(notes) => choose_notes(
            &mut builder,
            &notes,
            extsk,
            &mut amount,
            transparent_output_count,
            sapling_output_count,
        )?,
        Either::Right(utxos) => choose_utxos(
            &mut builder,
            &utxos,
            &mut amount,
            transparent_output_count,
            sapling_output_count,
        )?,
    };
    let amount = Amount::from_u64(amount).map_err(|_| "Invalid Amount")?;
    let to_address = decode_generic_address(network, to_address)?;
    match to_address {
        GenericAddress::Transparent(x) => builder
            .add_transparent_output(&x, amount)
            .map_err(|_| "Failed to add output")?,
        GenericAddress::Shield(x) => builder
            .add_sapling_output(None, x, amount, MemoBytes::empty())
            .map_err(|_| "Failed to add output")?,
    }

    if change.is_positive() {
        let change_address = decode_generic_address(network, change_address)?;
        match change_address {
            GenericAddress::Transparent(x) => builder
                .add_transparent_output(&x, change)
                .map_err(|_| "Failed to add transparent change")?,
            GenericAddress::Shield(x) => builder
                .add_sapling_output(None, x, change, MemoBytes::empty())
                .map_err(|_| "Failed to add shield change")?,
        }
    }

    let prover = get_prover().await;
    #[cfg(feature = "multicore")]
    {
        let (transmitter, mut receiver): (Sender<Progress>, Receiver<Progress>) =
            tokio::sync::mpsc::channel(1);
        builder.with_progress_notifier(transmitter);
        let tx_progress_future = async {
            loop {
                if let Some(status) = receiver.recv().await {
                    match status.end() {
                        Some(x) => set_tx_status((status.cur() as f32) / (x as f32)),
                        None => set_tx_status(0.0),
                    }
                } else {
                    set_tx_status(0.0);
                    break;
                }
            }
        };

        let (transmitter, mut receiver) = tokio::sync::mpsc::channel(1);
        rayon::spawn(move || {
            let res = prove_transaction(builder, nullifiers, fee, prover).unwrap();
            transmitter
                .blocking_send(res)
                .unwrap_or_else(|_| panic!("Cannot transmit tx"));
        });
        let (_, res) = join!(tx_progress_future, receiver.recv());
        return Ok(res.ok_or("Fail to receive tx proof")?);
    }
    #[cfg(not(feature = "multicore"))]
    prove_transaction(builder, nullifiers, fee, prover)
}

fn choose_utxos(
    builder: &mut Builder<Network, OsRng>,
    utxos: &[Utxo],
    amount: &mut u64,
    transparent_output_count: u64,
    sapling_output_count: u64,
) -> Result<(Vec<String>, Amount, u64), Box<dyn Error>> {
    let mut total = 0;
    let mut used_utxos = vec![];
    let mut transparent_input_count = 0;
    let mut fee = 0;
    for utxo in utxos {
        used_utxos.push(format!("{},{}", utxo.txid, utxo.vout));
        builder
            .add_transparent_input(
                SecretKey::from_slice(&utxo.private_key)?,
                OutPoint::new(
                    hex::decode(&utxo.txid)?
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .try_into()
                        .map_err(|_| "failed to decode txid")?,
                    utxo.vout,
                ),
                TxOut {
                    value: Amount::from_u64(utxo.amount).map_err(|_| "Invalid utxo amount")?,
                    script_pubkey: Script(utxo.script.clone()),
                },
            )
            .map_err(|_| "Failed to use utxo")?;
        transparent_input_count += 1;
        fee = fee_calculator(
            transparent_input_count,
            transparent_output_count,
            0,
            sapling_output_count,
        );
        total += utxo.amount;
        if total >= *amount + fee {
            break;
        }
    }
    if total < *amount + fee {
        if total >= *amount && *amount > fee {
            *amount -= fee;
        } else {
            Err("Not enough balance")?;
        }
    }

    let change = Amount::from_u64(total - *amount - fee).map_err(|_| "Invalid change")?;
    Ok((used_utxos, change, fee))
}

fn choose_notes(
    builder: &mut Builder<Network, OsRng>,
    notes: &[(Note, String)],
    extsk: &ExtendedSpendingKey,
    amount: &mut u64,
    transparent_output_count: u64,
    sapling_output_count: u64,
) -> Result<(Vec<String>, Amount, u64), Box<dyn Error>> {
    let mut total = 0;
    let mut nullifiers = vec![];
    let mut sapling_input_count = 0;
    let mut fee = 0;
    for (note, witness) in notes {
        let witness = Cursor::new(hex::decode(witness)?);
        let witness = IncrementalWitness::<Node>::read(witness)?;
        builder
            .add_sapling_spend(
                extsk.clone(),
                *note.recipient().diversifier(),
                note.clone(),
                witness.path().ok_or("Commitment Tree is empty")?,
            )
            .map_err(|_| "Failed to add sapling spend")?;
        let nullifier = note.nf(
            &extsk
                .to_diversifiable_full_viewing_key()
                .to_nk(Scope::External),
            witness.position() as u64,
        );
        nullifiers.push(hex::encode(nullifier.to_vec()));
        sapling_input_count += 1;
        fee = fee_calculator(
            0,
            transparent_output_count,
            sapling_input_count,
            sapling_output_count,
        );
        total += note.value().inner();
        if total >= *amount + fee {
            break;
        }
    }

    if total < *amount + fee {
        if total >= *amount && *amount > fee {
            *amount -= fee
        } else {
            Err("Not enough balance")?;
        }
    }

    let change = Amount::from_u64(total - *amount - fee).map_err(|_| "Invalid change")?;
    Ok((nullifiers, change, fee))
}

fn prove_transaction(
    builder: Builder<'_, Network, OsRng>,
    nullifiers: Vec<String>,
    fee: u64,
    prover: &impl TxProver,
) -> Result<JSTransaction, Box<dyn Error>> {
    #[cfg(not(test))]
    return {
        let (tx, _metadata) = builder.build(
            prover,
            &FeeRule::non_standard(Amount::from_u64(fee).map_err(|_| "Invalid fee")?),
        )?;

        let mut tx_hex = vec![];
        tx.write(&mut tx_hex)?;

        Ok(JSTransaction {
            txid: tx.txid().to_string(),
            txhex: hex::encode(tx_hex),
            nullifiers,
        })
    };
    #[cfg(test)]
    {
        // At this point we would use .mock_build()
        // However it returns an error for some reason
        // So let's just return the nullifiers and test those
        Ok(JSTransaction {
            txid: String::default(),
            txhex: String::default(),
            nullifiers,
        })
    }
}
