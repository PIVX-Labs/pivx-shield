pub use crate::keys::decode_extended_full_viewing_key;
pub use crate::keys::decode_extsk;
use crate::prover::get_prover;
use crate::prover::ImplTxProver;
use incrementalmerkletree::frontier::CommitmentTree;
use incrementalmerkletree::witness::IncrementalWitness;
use incrementalmerkletree::MerklePath;
pub use pivx_client_backend::decrypt_transaction;
pub use pivx_client_backend::keys::UnifiedFullViewingKey;
use pivx_primitives::consensus::Network;
use pivx_primitives::consensus::NetworkConstants;
pub use pivx_primitives::consensus::Parameters;
pub use pivx_primitives::consensus::{BlockHeight, MAIN_NETWORK, TEST_NETWORK};
use pivx_primitives::legacy::Script;
pub use pivx_primitives::memo::MemoBytes;
use pivx_primitives::merkle_tree::read_commitment_tree as zcash_read_commitment_tree;
use pivx_primitives::merkle_tree::read_incremental_witness;
use pivx_primitives::merkle_tree::write_commitment_tree;
use pivx_primitives::merkle_tree::write_incremental_witness;
use pivx_primitives::transaction::builder::BuildConfig;
pub use pivx_primitives::transaction::builder::Progress;
use pivx_primitives::transaction::components::transparent::builder::TransparentSigningSet;
use pivx_protocol::value::Zatoshis;
use secp256k1::Secp256k1;

use crate::keys::decode_generic_address;
use crate::keys::GenericAddress;
#[cfg(feature = "multicore")]
use atomic_float::AtomicF32;
pub use either::Either;
pub use pivx_primitives::transaction::builder::Builder;
pub use pivx_primitives::transaction::components::Amount;
use pivx_primitives::transaction::components::{OutPoint, TxOut};
pub use pivx_primitives::transaction::fees::fixed::FeeRule;
pub use pivx_primitives::transaction::Transaction;
pub use pivx_primitives::zip32::AccountId;
pub use pivx_primitives::zip32::Scope;
use rand_core::OsRng;
use sapling::zip32::ExtendedFullViewingKey;
pub use sapling::zip32::ExtendedSpendingKey;
pub use sapling::{note::Note, Node, Nullifier};

use sapling::NullifierDerivingKey;
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

#[cfg(feature = "multicore")]
type DefaultProgress = Sender;
#[cfg(not(feature = "multicore"))]
type DefaultProgress = ();

pub const DEPTH: u8 = 32;

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
    pub decrypted_notes: Vec<JSSpendableNote>,
    pub decrypted_new_notes: Vec<JSSpendableNote>,
    pub nullifiers: Vec<String>,
    pub commitment_tree: String,
    pub wallet_transactions: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Block {
    txs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct JSSpendableNote {
    note: Note,
    witness: String,
    nullifier: String,
}

pub struct SpendableNote {
    note: Note,
    witness: IncrementalWitness<Node, DEPTH>,
    nullifier: String,
}
impl SpendableNote {
    fn from_js_spendable_note(n: JSSpendableNote) -> Result<SpendableNote, Box<dyn Error>> {
        let wit = Cursor::new(hex::decode(n.witness)?);
        Ok(SpendableNote {
            note: n.note,
            witness: read_incremental_witness(wit)?,
            nullifier: n.nullifier,
        })
    }
    fn to_js_spendable_note(self) -> Result<JSSpendableNote, Box<dyn Error>> {
        let mut buff = Vec::new();
	write_incremental_witness(witness, &mut buff)?;
        Ok(JSSpendableNote {
            note: self.note,
            witness: hex::encode(&buff),
            nullifier: self.nullifier,
        })
    }
}

fn read_commitment_tree(tree_hex: &str) -> Result<CommitmentTree<Node, DEPTH>, Box<dyn Error>> {
    let buff = Cursor::new(hex::decode(tree_hex)?);
    Ok(zcash_read_commitment_tree(buff)?)
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
    let comp_note: Vec<JSSpendableNote> = serde_wasm_bindgen::from_value(comp_notes)?;
    let extfvk =
        decode_extended_full_viewing_key(enc_extfvk, is_testnet).map_err(|e| e.to_string())?;
    let key = UnifiedFullViewingKey::from_sapling_extended_full_viewing_key(extfvk)
        .map_err(|_|"Failed to create unified full viewing key")?;
    let mut comp_note = comp_note
        .into_iter()
        .map(|n| SpendableNote::from_js_spendable_note(n))
        .collect::<Result<Vec<SpendableNote>, _>>()
        .map_err(|e| e.to_string())?;
    let mut nullifiers = vec![];
    let mut new_notes = vec![];
    let mut wallet_transactions = vec![];
    for block in blocks {
        for tx in block.txs {
            let old_note_length = new_notes.len();
            let tx_nullifiers = handle_transaction(
                &mut tree,
                &tx,
                key.clone(),
                is_testnet,
                &mut comp_note,
                &mut new_notes,
            )
            .map_err(|_| "Couldn't handle transaction")?
            .into_iter()
            .map(|n| hex::encode(n.0))
            .collect::<Vec<_>>();
            let mut is_wallet_tx = old_note_length != new_notes.len();
            for n in comp_note.iter().chain(new_notes.iter()) {
                if is_wallet_tx || tx_nullifiers.contains(&n.nullifier) {
                    is_wallet_tx = true;
                    break;
                }
            }
            if is_wallet_tx {
                wallet_transactions.push(tx);
            }
            nullifiers.extend(tx_nullifiers);
        }
    }

    let ser_comp_note = serialize_comp_note(comp_note).map_err(|_| "couldn't decrypt notes")?;
    let ser_new_comp_note = serialize_comp_note(new_notes).map_err(|_| "couldn't decrypt notes")?;

    let mut buff = Vec::new();
    write_commitment_tree(&tree, &mut buff).map_err(|_| "Cannot write tree to buffer")?;

    Ok(serde_wasm_bindgen::to_value(&JSTxSaplingData {
        decrypted_notes: ser_comp_note,
        nullifiers,
        commitment_tree: hex::encode(buff),
        wallet_transactions,
        decrypted_new_notes: ser_new_comp_note,
    })?)
}

pub fn serialize_comp_note(
    comp_note: Vec<SpendableNote>,
) -> Result<Vec<JSSpendableNote>, Box<dyn Error>> {
    comp_note
        .into_iter()
        .map(|n| SpendableNote::to_js_spendable_note(n))
        .collect()
}

//add a tx to a given commitment tree and the return a witness to each output
pub fn handle_transaction(
    tree: &mut CommitmentTree<Node, DEPTH>,
    tx: &str,
    key: UnifiedFullViewingKey,
    is_testnet: bool,
    witnesses: &mut Vec<SpendableNote>,
    new_witnesses: &mut Vec<SpendableNote>,
) -> Result<Vec<Nullifier>, Box<dyn Error>> {
    let tx = Transaction::read(
        Cursor::new(hex::decode(tx)?),
        pivx_primitives::consensus::BranchId::Sapling,
    )?;
    let mut hash = HashMap::new();
    let nullif_key = key
        .sapling()
        .ok_or("Cannot generate nullifier key")?
        .to_nk(Scope::External);
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
            for &mut SpendableNote {
                ref mut witness, ..
            } in witnesses.iter_mut().chain(new_witnesses.iter_mut())
            {
                witness
                    .append(Node::from_cmu(out.cmu()))
                    .map_err(|_| "Failed to add cmu to witness")?;
            }
            for output in decrypted_tx.sapling_outputs() {
		let (note, index) = (output.note(), output.index());
                if index == i {
                    // Save witness
                    let witness = IncrementalWitness::<Node, DEPTH>::from_tree(tree.clone());
                    let nullifier = get_nullifier_from_note_internal(&nullif_key, &note, &witness)?;
                    new_witnesses.push(SpendableNote {
                        note: note.clone(),
                        witness,
                        nullifier,
                    });
                    break;
                }
            }
        }
    }
    Ok(nullifiers)
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
    let witness = Cursor::new(hex::decode(hex_witness).map_err(|e| e.to_string())?);

    let witness =
        read_incremental_witness(witness).map_err(|_| "Cannot read witness from buffer")?;
    let nullif_key = extfvk
        .to_diversifiable_full_viewing_key()
        .to_nk(Scope::External);
    let ser_nullifiers = get_nullifier_from_note_internal(&nullif_key, &note, &witness)
        .map_err(|e| e.to_string())?;
    Ok(serde_wasm_bindgen::to_value(&ser_nullifiers)?)
}

pub fn get_nullifier_from_note_internal(
    nullif_key: &NullifierDerivingKey,
    note: &Note,
    witness: &IncrementalWitness<Node, DEPTH>,
) -> Result<String, Box<dyn Error>> {
    let path = witness.path().ok_or("Cannot find witness path")?;
    Ok(hex::encode(note.nf(&nullif_key, path.position().into()).0))
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
    notes: Option<Vec<JSSpendableNote>>,
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
        let mut notes: Vec<(Note, String)> =
            notes.into_iter().map(|n| (n.note, n.witness)).collect();
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
    let mut builder = Builder::new(
        network,
        block_height,
        BuildConfig::Standard {
            sapling_anchor: None,
            orchard_anchor: None,
        },
    );

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
    let amount = Zatoshis::from_u64(amount).map_err(|_| "Invalid Amount")?;
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
    builder: &mut Builder<Network, DefaultProgress>,
    utxos: &[Utxo],
    amount: &mut u64,
    transparent_output_count: u64,
    sapling_output_count: u64,
) -> Result<(Vec<String>, Zatoshis, u64), Box<dyn Error>> {
    let mut total = 0;
    let mut used_utxos = vec![];
    let mut transparent_input_count = 0;
    let mut fee = 0;
    let secp = Secp256k1::new();
    for utxo in utxos {
        used_utxos.push(format!("{},{}", utxo.txid, utxo.vout));
        builder
            .add_transparent_input(
                SecretKey::from_slice(&utxo.private_key)?.public_key(&secp),
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
                    value: Zatoshis::from_u64(utxo.amount).map_err(|_| "Invalid utxo amount")?,
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

    let change = Zatoshis::from_u64(total - *amount - fee).map_err(|_| "Invalid change")?;
    Ok((used_utxos, change, fee))
}

fn choose_notes(
    builder: &mut Builder<Network, DefaultProgress>,
    notes: &[(Note, String)],
    extsk: &ExtendedSpendingKey,
    amount: &mut u64,
    transparent_output_count: u64,
    sapling_output_count: u64,
) -> Result<(Vec<String>, Zatoshis, u64), Box<dyn Error>> {
    let mut total = 0;
    let mut nullifiers = vec![];
    let mut sapling_input_count = 0;
    let mut fee = 0;
    for (note, witness) in notes {
        let witness = Cursor::new(hex::decode(witness)?);

        let witness = read_incremental_witness::<Node, _, DEPTH>(witness)?;
        builder
            .add_sapling_spend(
                extsk.to_diversifiable_full_viewing_key().fvk().clone(),
                note.clone(),
                witness.path().ok_or("Commitment Tree is empty")?,
            )
            .map_err(|_| "Failed to add sapling spend")?;
        let nullifier = note.nf(
            &extsk
                .to_diversifiable_full_viewing_key()
                .to_nk(Scope::External),
            witness.witnessed_position().into(),
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

    let change = Zatoshis::from_u64(total - *amount - fee).map_err(|_| "Invalid change")?;
    Ok((nullifiers, change, fee))
}

fn prove_transaction(
    builder: Builder<'_, Network, DefaultProgress>,
    extsk: ExtendedSpendingKey,
    transparent_keys: &TransparentSigningSet,
    nullifiers: Vec<String>,
    fee: u64,
    prover: &ImplTxProver,
) -> Result<JSTransaction, Box<dyn Error>> {
    #[cfg(not(test))]
    return {
        let result = builder.build(
            transparent_keys,
            &[extsk],
            &[],
            OsRng,
            &prover.1,
            &prover.0,
            &FeeRule::non_standard(Zatoshis::from_u64(fee).map_err(|_| "Invalid fee")?),
        )?;

        let mut tx_hex = vec![];
	let tx = result.transaction();
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
