use serde::{Deserialize, Serialize};

use pivx_primitives::consensus::{Parameters, MAIN_NETWORK, TEST_NETWORK};
use wasm_bindgen::prelude::*;

use pivx_primitives::sapling::PaymentAddress;
use pivx_primitives::zip32::sapling::ExtendedSpendingKey;
use pivx_primitives::zip32::AccountId;
use pivx_primitives::zip32::DiversifierIndex;

use pivx_client_backend::encoding;
use pivx_client_backend::encoding::decode_payment_address;
use pivx_client_backend::encoding::decode_transparent_address;
use pivx_client_backend::keys::sapling;
use pivx_primitives::consensus::Network;
use pivx_primitives::legacy::TransparentAddress;
use std::error::Error;
// Data needed to generate an extended spending key
#[derive(Serialize, Deserialize)]
pub struct JSExtendedSpendingKeySerData {
    pub seed: [u8; 32],
    pub coin_type: u32,
    pub account_index: u32,
}

pub enum GenericAddress {
    Shield(PaymentAddress),
    Transparent(TransparentAddress),
}

pub fn decode_generic_address(
    network: Network,
    enc_addr: &str,
) -> Result<GenericAddress, Box<dyn Error>> {
    if enc_addr.starts_with(network.hrp_sapling_payment_address()) {
        let to_address = decode_payment_address(network.hrp_sapling_payment_address(), enc_addr)
            .map_err(|_| "Failed to decode sending address")?;
        Ok(GenericAddress::Shield(to_address))
    } else {
        let to_address = decode_transparent_address(
            &network.b58_pubkey_address_prefix(),
            &network.b58_script_address_prefix(),
            enc_addr,
        )?
        .ok_or("Failed to decode transparent address")?;
        Ok(GenericAddress::Transparent(to_address))
    }
}
pub fn decode_extsk(enc_extsk: &str, is_testnet: bool) -> Result<ExtendedSpendingKey, Box<dyn Error>> {
    let enc_str: &str = if is_testnet {
        TEST_NETWORK.hrp_sapling_extended_spending_key()
    } else {
        MAIN_NETWORK.hrp_sapling_extended_spending_key()
    };

    Ok(encoding::decode_extended_spending_key(enc_str, enc_extsk).map_err(|_| "Cannot decde extsk")?)
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
pub fn generate_extended_spending_key_from_seed(val: JsValue) -> Result<JsValue, JsValue> {
    let data_arr: JSExtendedSpendingKeySerData = serde_wasm_bindgen::from_value(val)?;
    let extsk = sapling::spending_key(
        &data_arr.seed,
        data_arr.coin_type,
        AccountId::from(data_arr.account_index),
    );
    let enc_extsk = encode_extsk(&extsk, data_arr.coin_type == 1);
    Ok(serde_wasm_bindgen::to_value(&enc_extsk)?)
}

#[derive(Debug, Serialize, Deserialize)]
struct NewAddress {
    pub address: String,
    pub diversifier_index: Vec<u8>,
}
//Generate the deafult address of a given encoded extended full viewing key
#[wasm_bindgen]
pub fn generate_default_payment_address(enc_extsk: String, is_testnet: bool) -> Result<JsValue, JsValue> {
    let extsk = decode_extsk(&enc_extsk, is_testnet).map_err(|e| e.to_string())?;
    let (def_index, def_address) = extsk.to_diversifiable_full_viewing_key().default_address();
    Ok(serde_wasm_bindgen::to_value(&NewAddress {
        address: encode_payment_address(&def_address, is_testnet),
        diversifier_index: def_index.0.to_vec(),
    })?)
}
// Generate the n_address-th valid payment address given the encoded extended full viewing key and a starting index
#[wasm_bindgen]
pub fn generate_next_shielding_payment_address(
    enc_extsk: String,
    diversifier_index: &[u8],
    is_testnet: bool,
) -> Result<JsValue, JsValue> {
    let extsk = decode_extsk(&enc_extsk, is_testnet).map_err(|e| e.to_string())?;
    let mut diversifier_index = DiversifierIndex(
        diversifier_index
            .try_into()
            .map_err(|_| "Invalid diversifier index")?,
    );
    diversifier_index
        .increment()
        .map_err(|_| "No valid indeces left")?;
    let (new_index, address) = extsk
        .to_diversifiable_full_viewing_key()
        .find_address(diversifier_index)
        .ok_or("No valid indeces left")?; // There are so many valid addresses this should never happen

    Ok(serde_wasm_bindgen::to_value(&NewAddress {
        address: encode_payment_address(&address, is_testnet),
        diversifier_index: new_index.0.to_vec(),
    })?)
}
