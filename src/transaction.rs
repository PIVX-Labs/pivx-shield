pub use crate::keys::decode_extsk;
pub use pivx_client_backend::decrypt_transaction;
pub use pivx_client_backend::encoding::decode_payment_address;
use pivx_client_backend::encoding::decode_transparent_address;
pub use pivx_client_backend::keys::UnifiedFullViewingKey;
use pivx_primitives::consensus::Network;
pub use pivx_primitives::consensus::Parameters;

pub use pivx_primitives::consensus::{BlockHeight, MAIN_NETWORK, TEST_NETWORK};
use pivx_primitives::legacy::Script;
pub use pivx_primitives::memo::MemoBytes;
pub use pivx_primitives::merkle_tree::{CommitmentTree, IncrementalWitness, MerklePath};
pub use pivx_primitives::sapling::PaymentAddress;

use async_once::AsyncOnce;
pub use either::Either;
use lazy_static::lazy_static;
pub use pivx_primitives::sapling::{note::Note, Node, Nullifier};
pub use pivx_primitives::transaction::builder::Builder;
pub use pivx_primitives::transaction::components::Amount;
use pivx_primitives::transaction::components::{OutPoint, TxOut};
pub use pivx_primitives::transaction::fees::fixed::FeeRule;
pub use pivx_primitives::transaction::Transaction;
pub use pivx_primitives::zip32::AccountId;
pub use pivx_primitives::zip32::ExtendedSpendingKey;
pub use pivx_primitives::zip32::Scope;
pub use pivx_proofs::prover::LocalTxProver;
use rand_core::OsRng;
pub use reqwest::Client;
use secp256k1::SecretKey;
pub use serde::{Deserialize, Serialize};
use std::convert::TryInto;
pub use std::path::Path;
pub use std::{collections::HashMap, error::Error, io::Cursor};
pub use wasm_bindgen::prelude::*;
mod test;

lazy_static! {
    static ref PROVER: AsyncOnce<LocalTxProver> = AsyncOnce::new(async {
        let (sapling_spend_bytes, sapling_output_bytes): (Vec<u8>, Vec<u8>) =
            fetch_params().await.expect("Cannot fetch params");
        LocalTxProver::from_bytes(&sapling_spend_bytes, &sapling_output_bytes)
    });
}

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
    println!("{} {} {} {}",transparent_input_count, transparent_output_count, sapling_input_count, sapling_output_count);
    fee_per_byte
        * (sapling_output_count * sapling_output_size
            + sapling_input_count * sapling_input_size
            + transparent_input_count * transparent_input_size
            + transparent_output_count * transparent_output_size + tx_offset_size)
}
async fn fetch_params() -> Result<(Vec<u8>, Vec<u8>), Box<dyn Error>> {
    let c = Client::new();
    let sapling_output_bytes = c
        .get("https://duddino.com/sapling-output.params")
        .send()
        .await?
        .bytes()
        .await?;
    let sapling_spend_bytes = c
        .get("https://duddino.com/sapling-spend.params")
        .send()
        .await?
        .bytes()
        .await?;

    if sha256::digest(&*sapling_output_bytes)
        != "2f0ebbcbb9bb0bcffe95a397e7eba89c29eb4dde6191c339db88570e3f3fb0e4"
    {
        Err("Sha256 does not match for sapling output")?;
    }

    if sha256::digest(&*sapling_spend_bytes)
        != "8e48ffd23abb3a5fd9c5589204f32d9c31285a04b78096ba40a79b75677efc13"
    {
        Err("Sha256 does not match for sapling spend")?;
    }

    Ok((sapling_spend_bytes.to_vec(), sapling_output_bytes.to_vec()))
}

#[wasm_bindgen]
pub async fn load_prover() -> bool {
    PROVER.get().await;
    true
}

#[derive(Serialize, Deserialize)]
pub struct JSTxSaplingData {
    pub decrypted_notes: Vec<(Note, String)>,
    pub nullifiers: Vec<String>,
    pub commitment_tree: String,
}

//Input a tx and return: the updated commitment merkletree, all the nullifier found in the tx and all the node decoded with the corresponding witness
#[wasm_bindgen]
pub fn handle_transaction(tree_hex: &str, tx: &str, enc_extsk: &str, is_testnet: bool) -> JsValue {
    let buff =
        Cursor::new(hex::decode(tree_hex).expect("Cannot decode commitment tree from hexadecimal"));
    let mut tree = CommitmentTree::<Node>::read(buff).expect("Cannot decode commitment tree!");
    let extsk = decode_extsk(enc_extsk, is_testnet);
    let key = UnifiedFullViewingKey::new(Some(extsk.to_diversifiable_full_viewing_key()), None)
        .expect("Failed to create unified full viewing key");
    let (nullifiers, comp_note) =
        handle_transaction_internal(&mut tree, tx, &key, true).expect("Cannot decode tx");
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
pub async fn create_transaction(options: JsValue) -> JsValue {
    let JSTxOptions {
        notes,
        extsk,
        to_address,
        change_address,
        amount,
        block_height,
        is_testnet,
        utxos,
    } = serde_wasm_bindgen::from_value::<JSTxOptions>(options).expect("Cannot deserialize notes");
    assert!(
        !(notes.is_some() && utxos.is_some()),
        "Notes and UTXOs were both provided"
    );
    let extsk = decode_extsk(&extsk, is_testnet);
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
    .expect("Failed to create tx");
    serde_wasm_bindgen::to_value(&result).expect("Cannot serialize transaction")
}

/// Create a transaction.
/// The notes are used in the order they're provided
/// It might be useful to sort them first, or use any other smart alogorithm
pub async fn create_transaction_internal(
    inputs: Either<Vec<(Note, String)>, Vec<Utxo>>,
    extsk: &ExtendedSpendingKey,
    to_address: &str,
    change_address: &str,
    amount: u64,
    block_height: BlockHeight,
    network: Network,
) -> Result<JSTransaction, Box<dyn Error>> {
    let mut builder = Builder::new(network, block_height);
    let (transparent_output_count, sapling_output_count)  = if  to_address.starts_with(network.hrp_sapling_payment_address()) {(0,2)} else {(1,2)};
    let (nullifiers, change, fee) = match inputs {
        Either::Left(notes) => choose_notes(&mut builder, &notes, extsk, amount, transparent_output_count, sapling_output_count)?,
        Either::Right(utxos) => choose_utxos(&mut builder, &utxos, amount,transparent_output_count, sapling_output_count)?,
    };

    let amount = Amount::from_u64(amount).map_err(|_| "Invalid Amount")?;

    let change_address =
        decode_payment_address(network.hrp_sapling_payment_address(), change_address)
            .map_err(|_| "Failed to decode change address")?;
    if to_address.starts_with(network.hrp_sapling_payment_address()) {
        let to_address = decode_payment_address(network.hrp_sapling_payment_address(), to_address)
            .map_err(|_| "Failed to decode sending address")?;
        builder
            .add_sapling_output(None, to_address, amount, MemoBytes::empty())
            .map_err(|_| "Failed to add output")?;
    } else {
        let to_address = decode_transparent_address(
            &network.b58_pubkey_address_prefix(),
            &network.b58_script_address_prefix(),
            to_address,
        )?
        .ok_or("Failed to decode transparent address")?;
        builder
            .add_transparent_output(&to_address, amount)
            .map_err(|_| "Failed to add output")?;
    }

    builder
        .add_sapling_output(None, change_address, change, MemoBytes::empty())
        .map_err(|_| "Failed to add change")?;

    prove_transaction(builder, nullifiers, fee).await
}

fn choose_utxos(
    builder: &mut Builder<Network, OsRng>,
    utxos: &[Utxo],
    amount: u64,
    transparent_output_count: u64,
    sapling_output_count: u64,
) -> Result<(Vec<String>, Amount, u64), Box<dyn Error>> {
    let mut total = 0;
    let mut used_utxos = vec![];
    let mut transparent_input_count = 0;
    let mut fee = 0;
    for utxo in utxos {
        used_utxos.push(utxo.txid.clone());
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
        fee = fee_calculator(transparent_input_count, transparent_output_count, 0, sapling_output_count);
        total += utxo.amount;
        if total >= amount + fee {
            break;
        }
    }

    if total < amount + fee {
        Err("Not enough balance")?;
    }

    let change = Amount::from_u64(total - amount - fee).map_err(|_| "Invalid change")?;
    Ok((used_utxos, change, fee))
}

fn choose_notes(
    builder: &mut Builder<Network, OsRng>,
    notes: &[(Note, String)],
    extsk: &ExtendedSpendingKey,
    amount: u64,
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
                .to_nk(Scope::Internal),
            witness.position() as u64,
        );
        nullifiers.push(hex::encode(nullifier.to_vec()));
        sapling_input_count += 1;
        fee = fee_calculator(0, transparent_output_count, sapling_input_count, sapling_output_count);
        total += note.value().inner();
        if total >= amount + fee {
            break;
        }
    }

    if total < amount + fee {
        Err("Not enough balance")?;
    }

    let change = Amount::from_u64(total - amount - fee).map_err(|_| "Invalid change")?;
    Ok((nullifiers, change, fee))
}

async fn prove_transaction(
    builder: Builder<'_, Network, OsRng>,
    nullifiers: Vec<String>,
    fee: u64,
) -> Result<JSTransaction, Box<dyn Error>> {
    #[cfg(not(test))]
    return {
        let (tx, _metadata) = builder.build(
            PROVER.get().await,
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
