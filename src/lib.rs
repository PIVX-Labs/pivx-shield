mod utils;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use pivx_primitives::consensus::Parameters;
use pivx_primitives::consensus::{MainNetwork, TestNetwork, MAIN_NETWORK, TEST_NETWORK};
use pivx_primitives::zip32::sapling::ExtendedSpendingKey;
use pivx_primitives::zip32::AccountId;
use pivx_primitives::zip32::DiversifierIndex;

use pivx_client_backend::encoding;
use pivx_client_backend::keys::sapling; //::{decode_extended_spending_key, decode_transparent_address};
                                        // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
                                        // allocator.
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

#[wasm_bindgen]
pub fn greet() {
    alert("Hello, pivx-shielding!");
}
