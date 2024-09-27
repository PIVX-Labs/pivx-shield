use crate::utils::set_panic_hook;
use wasm_bindgen::prelude::*;

use crate::mainnet_checkpoints::MAINNET_CHECKPOINTS;
use crate::testnet_checkpoints::TESTNET_CHECKPOINTS;

//Return the closest checkpoint
pub fn get_checkpoint(block_height: i32, is_testnet: bool) -> (i32, &'static str) {
    // this is a decent place for initializing the panic hook as it's always going to be
    // Run when the library is initialized
    set_panic_hook();

    let used_checkpoints = if is_testnet {
        TESTNET_CHECKPOINTS
    } else {
        MAINNET_CHECKPOINTS
    };

    used_checkpoints
        .iter()
        .rev()
        .find(|x| x.0 <= block_height)
        .copied()
        .unwrap_or(used_checkpoints[0])
}

//TODO: update once we have more checkpoints on testnet
#[cfg(test)]
mod test {
    use crate::checkpoint::get_checkpoint;
    use pivx_primitives::merkle_tree::CommitmentTree;
    use pivx_primitives::sapling::Node;
    use std::error::Error;
    use std::io::Cursor;
    #[test]
    fn check_testnet_checkpoints() -> Result<(), Box<dyn Error>> {
        // Blocks above last checkpoint should yield last checkpoint
        assert_eq!(get_checkpoint(1123200 + 30000, true).0, 1123200);
        // Blocks equal to last checkpoint should yield last checkpoint
        assert_eq!(get_checkpoint(1123200, true).0, 1123200);
        // Blocks in between two adjacent checkpoints should yield the smaller of the two
        assert_eq!(get_checkpoint((907200 + 950400) / 2, true).0, 907200);
        // Block 0 should yield an empty commitment tree
        let tree = Cursor::new(hex::decode(get_checkpoint(0, true).1)?);
        let tree = CommitmentTree::<Node>::read(tree)?;
        assert_eq!(tree, CommitmentTree::empty());
        Ok(())
    }
    #[test]
    fn check_mainnet_checkpoints() -> Result<(), Box<dyn Error>> {
        // Blocks below shield activation height should return first checkpoint
        assert_eq!(get_checkpoint(2700000 - 1, false).0, 2700000);
        // Blocks equal to shield activation height should return first checkpoint
        assert_eq!(get_checkpoint(2700000, false).0, 2700000);
        // Blocks near shield activation height should return first checkpoint
        assert_eq!(get_checkpoint(2700001, false).0, 2700000);
        assert_eq!(get_checkpoint(3758400 + 1, false).0, 3758400);
        // Blocks in between two adjacent checkpoints should yield the smaller of the two
        assert_eq!(get_checkpoint((3758400 + 3715200) / 2, false).0, 3715200);
        // First checkpoint should return an empty commitment tree
        let buff = Cursor::new(hex::decode(get_checkpoint(2700000, false).1)?);
        let tree = CommitmentTree::<Node>::read(buff)?;
        assert_eq!(tree, CommitmentTree::empty());
        Ok(())
    }
}

//Output the closest checkpoint to a given blockheight
#[wasm_bindgen]
pub fn get_closest_checkpoint(block_height: i32, is_testnet: bool) -> Result<JsValue, JsValue> {
    Ok(serde_wasm_bindgen::to_value(&get_checkpoint(
        block_height,
        is_testnet,
    ))?)
}
