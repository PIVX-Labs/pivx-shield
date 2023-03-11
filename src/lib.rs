mod checkpoint;
mod utils;

use crate::checkpoint::get_checkpoint;

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, io::Cursor};
use wasm_bindgen::prelude::*;

use pivx_primitives::consensus::{BlockHeight, Parameters, MAIN_NETWORK, TEST_NETWORK};
use pivx_primitives::merkle_tree::{CommitmentTree, IncrementalWitness, MerklePath};
use pivx_primitives::sapling::{note::Note, Node, Nullifier};
use pivx_primitives::transaction::Transaction;
use pivx_primitives::zip32::sapling::ExtendedSpendingKey;
use pivx_primitives::zip32::AccountId;
use pivx_primitives::zip32::DiversifierIndex;

use pivx_client_backend::decrypt_transaction;
use pivx_client_backend::encoding;
use pivx_client_backend::keys::{sapling, UnifiedFullViewingKey};
use pivx_primitives::sapling::PaymentAddress;
use pivx_primitives::zip32::Scope;

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

#[derive(Serialize, Deserialize)]
pub struct JSTxSaplingData {
    pub decrypted_notes: Vec<(Note, String)>,
    pub nullifiers: Vec<String>,
    pub commitment_tree: String,
}

fn decode_extsk(enc_extsk: &str, is_testnet: bool) -> ExtendedSpendingKey {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };
    let enc_str: &str = "p-secret-spending-key-test"; // ONLY FOR TESTING
    return encoding::decode_extended_spending_key(enc_str, enc_extsk).expect("Cannot decde extsk");
}

fn encode_extsk(extsk: &ExtendedSpendingKey, is_testnet: bool) -> String {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };
    let enc_str: &str = "p-secret-spending-key-test"; // ONLY FOR TESTING
    return encoding::encode_extended_spending_key(enc_str, extsk);
}

fn encode_payment_address(addr: &PaymentAddress, is_testnet: bool) -> String {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_payment_address()
    } else {
        MAIN_NETWORK.hrp_sapling_payment_address()
    };
    return encoding::encode_payment_address(enc_str, addr);
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
    let enc_extsk = encode_extsk(&extsk, data_arr.coin_type == 1);
    return serde_wasm_bindgen::to_value(&enc_extsk)
        .expect("Cannot serialize extended spending key");
}

//Generate the n_address-th valid payment address given the encoded extended full viewing key and a starting index
#[wasm_bindgen]
pub fn generate_next_shielding_payment_address(
    enc_extsk: String,
    n_address: i32,
    is_testnet: bool,
) -> JsValue {
    let extsk = decode_extsk(&enc_extsk, is_testnet);
    let mut found_addresses = 0;
    let mut diversifier_index = DiversifierIndex::new();
    loop {
        let payment_address = extsk
            .to_extended_full_viewing_key()
            .find_address(diversifier_index);
        if let Some(payment_address) = payment_address {
            found_addresses += 1;
            if found_addresses == n_address {
                let enc_addr = encode_payment_address(&payment_address.1, is_testnet);
                return serde_wasm_bindgen::to_value(&enc_addr)
                    .expect("Cannot serialize payment address");
            }
        }
        diversifier_index.increment();
    }
}

//Output the closest checkpoint to a given blockheight
#[wasm_bindgen]
pub fn get_closest_checkpoint(block_height: i32, is_testnet: bool) -> JsValue {
    return serde_wasm_bindgen::to_value(&get_checkpoint(block_height, is_testnet).unwrap())
        .expect("Cannot serialize checkpoint");
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
    for x in comp_note.iter() {
        let mut buff = Cursor::new(Vec::new());
        x.1.write(&mut buff)
            .expect("Cannot write witness to buffer");
        let newNote = x.0.clone();
        ser_comp_note.push((newNote, hex::encode(&buff.into_inner())));
    }

    for nullif in nullifiers.iter() {
        ser_nullifiers.push(hex::encode(nullif.0));
    }

    let mut buff = Cursor::new(Vec::new());
    tree.write(&mut buff).expect("Cannot write tree to buffer");

    let res: JSTxSaplingData = JSTxSaplingData {
        decrypted_notes: ser_comp_note,
        nullifiers: ser_nullifiers,
        commitment_tree: hex::encode(buff.into_inner()),
    };
    return serde_wasm_bindgen::to_value(&res).expect("Cannot serialize tx output");
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
pub fn remove_unspent_notes(
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

    for x in hex_notes.iter() {
        let buff = Cursor::new(hex::decode(&x.1).expect("Cannot decode witness"));
        let newNote = x.0.clone();
        let path = IncrementalWitness::<Node>::read(buff)
            .expect("Cannot read witness from buffer")
            .path()
            .expect("Cannot find witness path");
        notes.push((newNote, x.1.clone(), path));
    }
    for x in notes.iter() {
        let nf = hex::encode(x.0.nf(&nullif_key, x.2.position).0);
        if nullifiers.iter().filter(|x| **x == nf).next() == None {
            unspent_notes.push((x.0.clone(), x.1.clone()));
        };
    }
    serde_wasm_bindgen::to_value(&unspent_notes).expect("Cannot serialize unspent notes")
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, pivx-shielding!");
}

#[cfg(test)]
mod test {
    use crate::handle_transaction_internal;
    use jubjub::Fr;
    use pivx_client_backend::encoding;
    use pivx_client_backend::keys::UnifiedFullViewingKey;
    use pivx_primitives::consensus::Parameters;
    use pivx_primitives::consensus::TEST_NETWORK;
    use pivx_primitives::merkle_tree::CommitmentTree;
    use pivx_primitives::sapling::value::NoteValue;
    use pivx_primitives::sapling::Node;
    use pivx_primitives::sapling::Note;
    use pivx_primitives::sapling::Rseed::BeforeZip212;
    #[test]
    fn check_tx_decryption() {
        let mut tree = CommitmentTree::<Node>::empty();
        //TODO: remove the hardcoded bench32 value as soon as the pivx lib is updated; use TEST_NETWORK.hrp_sapling_extended_spending_key() instead.
        //This This pair (tx,key) has been generated on regtest and contains 1 shield note
        let skey = encoding::decode_extended_spending_key( "p-secret-spending-key-test", "p-secret-spending-key-test1qd7a5dwjqqqqpqyzy3xs3usw7rzal27gvx6szvt56qff69ceqxtzdst9cuvut3n7dcp28wk2why35qd3989hdvf5wq9m62q6xfrmnlkf0r70v2s7x63sr2zzt8shr6psry8sq66kvzwskrghutgd7wmqknsljq0j0t2kmyg8xzqweug0pg40ml0s8mvvmgmp9c9pdvmpnx9gnhwde9yrg4f2c36c808d6p29ywevmn47lp9elyawr93wxl96ttd5pevj6f68qc6rcps5u9990").expect("Cannot decode spending key");
        let tx = "0300000001a7d31ea039ab2a9914be2a84b6e8966758da5f8d1a64ac6fb49d2763dccc38da000000006b483045022100bb67345313edea3c7462c463ea8e03ef3b14caccfbefff9877ef246138427b6a02200b74211e1f27be080561c3985980e6b0e2e833f0751ea68dfb1e465b994afefc0121025c6802ec58464d8e65d5f01be0b7ce6e8404e4a99f28ea3bfe47efe40df9108cffffffff01e8657096050000001976a914b4f73d5c66d999699a4c38ba9fe851d7141f1afa88ac0000000001003665c4ffffffff00010b39ab5d98de6f5e3f50f3f075f61fea263b4cdd6927a53ac92c196be72911237f5041af34fed06560b8620e16652edf6d297d14a9cff2145731de6643d4bf13e189dbc4b6c4b91fe421133a2f257e5b516efd9b080814251ec0169fabdac1ce4a14575d3a42a7ca852c1ef6f6e1f3daf60e9ae4b77ef4d9a589dcbc09e8437fc28e80d6a0c4f1627b3e34ee9dd5cd587d1d57bab30e0a2eba893a6b61d7e53f5b49b4cb67a807e5db203b76744025d8395c83be2eb71009f9b82e78e7b65d9740340106ee59b22cd3628f1f10c3712c2b4f86464b627b27910cd3e0a80c5387798db4f15f751b5886beb1ab1a8c298185ed6f5d3a074689ba6e327e8dc2bd3b41790ecbe0240f909b8735b8ac98a59855b448e9f37d31d5d25b71959264c145abd15f0606ab5844391819afd4017890696272abad451dab8654d76e41c389941f0fd134d7d6e3b971b15cc63ba9bea421383639bdbeaa970636d637a1c6167154f39ded089d0f07776c58e8e86c0dac8259d22644e9d8a89456e9ccf2f66ce8633a9055f1703669c6a7b009865347ef608cb4ba8f3158e05947580ec50c32f69c0079dff58b3b53367f43490d4bcaba946ef4c42b4d366c66184f84ec442499a056b6b60eeaee94151459ac0b61eb6debfa96554bbe8ec39d2c49ee6eca48ed8dc137f84584803e2372ec35e0f9f4252beef9170419e703183fa87e7d35c2403b41700bc9f5d69da6c01c870515694f5c48372cba6bacd6a79ca1cdb85f38841f7680d0dd6853b22fc95d6e307419271edb05f2f40733c31c6f827eca592658716c5c73a9dd00a7e387250beffaa78bd1f104e031e00f014f9a50935864e11ffd655ea4d4c6c3d80b681e7581a19b2668c00528110ee5322add9dacb35b519280812050061788884cad7cc409a9261e86485cc4f2d904bdf40b3c78208a395a2488eb938b8a198b51ac418fa79e5d1d7bd8f96fe0910fe61136d8fe302f144745a988d6de83e89cd8befef8a762103aa32a14d93e3ac41b44188ab385b65c1f21cf29f19a6d2af556385dd60a994ecd1ac909488f7abce29e26690651a389d4466a9e20b7f08bfbdf4f4aa3e1577dc7debf1951688db8c75347d01e836f7816df3c7a7aaa833cbd6309d179d5dfc34045e52984cf475890f04b2ffcdf123175cc07568d08d9b8525ad9eabad231e1549a19fdce0fbb30c1fe7ec59bf8ed8e642ec6571456bdba8ade4458cffb1d65fee35242d7409de14a21514416a68e9c2d5c21eb9ca5813e1d8162a48d650ed7696b7b14b4f3d3f5eb892cf32614f62dea794e7f68e6d3d3ae6edf22e811f85e1ac7fe2a8437bdf287aa4d5ff842173039074516304042370a4e2feeb901319665ffc9b005b37c2afbea22faca316ea4f6b5f365fe46f679581966dadd029d687d2b400201";
        let key = UnifiedFullViewingKey::new(Some(skey.to_diversifiable_full_viewing_key()), None)
            .expect("Failed to create key");
        let (nullifiers, comp_note) =
            handle_transaction_internal(&mut tree, tx, &key, true).unwrap();
        //This was a t-s tx
        assert_eq!(nullifiers.len(), 0);
        //Successfully decrypt exactly 1 note
        assert_eq!(comp_note.len(), 1);
        let note = &comp_note[0].0;
        //Successfully decrypt the balance
        assert_eq!(note.value().inner(), 1000000000);
        //Successfully decrypt the payment address
        assert_eq!(encoding::encode_payment_address(TEST_NETWORK.hrp_sapling_payment_address(),&note.recipient()),"ptestsapling1nkwhh4umfnze4u6wz4fgdy7hfrgp6hn7efc75xwwau6qdmmd79epgqmc9dqc58j7sffmy0lzhe7");
    }

    #[test]
    pub fn check_note_serialization() {
        let skey = encoding::decode_extended_spending_key( "p-secret-spending-key-test", "p-secret-spending-key-test1qd7a5dwjqqqqpqyzy3xs3usw7rzal27gvx6szvt56qff69ceqxtzdst9cuvut3n7dcp28wk2why35qd3989hdvf5wq9m62q6xfrmnlkf0r70v2s7x63sr2zzt8shr6psry8sq66kvzwskrghutgd7wmqknsljq0j0t2kmyg8xzqweug0pg40ml0s8mvvmgmp9c9pdvmpnx9gnhwde9yrg4f2c36c808d6p29ywevmn47lp9elyawr93wxl96ttd5pevj6f68qc6rcps5u9990").expect("Cannot decode spending key");

        let test_ser = Note::from_parts(
            skey.default_address().1,
            NoteValue::from_raw(10),
            BeforeZip212(Fr::one()),
        );
        let ser = serde_json::to_value(&test_ser).expect("Cannot serialize note");
        let test_deser: Note = serde_json::from_value(ser).expect("Cannot deserialize note");
        assert_eq!(test_deser.value(), test_ser.value());
        assert_eq!(test_deser.recipient(), test_ser.recipient());
        if let BeforeZip212(rseed) = test_deser.rseed() {
            assert_eq!(rseed, &Fr::one());
        } else {
            panic!();
        }
    }
}
