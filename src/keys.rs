use serde::{Deserialize, Serialize};

use pivx_primitives::consensus::{Parameters, MAIN_NETWORK, TEST_NETWORK};
use wasm_bindgen::prelude::*;

use pivx_primitives::sapling::PaymentAddress;
use pivx_primitives::zip32::sapling::ExtendedSpendingKey;
use pivx_primitives::zip32::AccountId;
use pivx_primitives::zip32::DiversifierIndex;

use pivx_client_backend::encoding;
use pivx_client_backend::keys::sapling;

// Data needed to generate an extended spending key
#[derive(Serialize, Deserialize)]
pub struct JSExtendedSpendingKeySerData {
    pub seed: [u8; 32],
    pub coin_type: u32,
    pub account_index: u32,
}

pub fn decode_extsk(enc_extsk: &str, is_testnet: bool) -> ExtendedSpendingKey {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };

    encoding::decode_extended_spending_key(enc_str, enc_extsk).expect("Cannot decde extsk")
}

pub fn encode_extsk(extsk: &ExtendedSpendingKey, is_testnet: bool) -> String {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };

    encoding::encode_extended_spending_key(enc_str, extsk)
}

pub fn encode_payment_address(addr: &PaymentAddress, is_testnet: bool) -> String {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_payment_address()
    } else {
        MAIN_NETWORK.hrp_sapling_payment_address()
    };
    encoding::encode_payment_address(enc_str, addr)
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
    serde_wasm_bindgen::to_value(&enc_extsk).expect("Cannot serialize extended spending key")
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
        let bundle_adress = extsk
            .to_diversifiable_full_viewing_key()
            .find_address(diversifier_index);
        if let Some((new_diversifier_index, payment_address)) = bundle_adress {
            let enc_addr = encode_payment_address(&payment_address, is_testnet);
            diversifier_index = new_diversifier_index;
            found_addresses += 1;
            if found_addresses == n_address {
                return serde_wasm_bindgen::to_value(&enc_addr)
                    .expect("Cannot serialize payment address");
            }
        }
        diversifier_index
            .increment()
            .expect("Failed to increment the index");
    }
}
