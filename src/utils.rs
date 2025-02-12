use std::io::Cursor;

use pivx_primitives::merkle_tree::{read_commitment_tree, HashSer};
use sapling::Node;
use wasm_bindgen::prelude::*;

use crate::transaction::DEPTH;

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn get_sapling_root(tree_hex: &str) -> Result<JsValue, JsValue> {
    let buff = Cursor::new(
        hex::decode(tree_hex).map_err(|_| "Cannot decode commitment tree from hexadecimal")?,
    );

    let tree = read_commitment_tree::<Node, _, DEPTH>(buff)
        .map_err(|_| "Cannot decode commitment tree!")?;
    let mut root = Vec::new();
    tree.root()
        .write(&mut root)
        .map_err(|_| "Cannot write sapling root")?;
    Ok(JsValue::from_str(&hex::encode(root)))
}
