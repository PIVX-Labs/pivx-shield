//Return the closest checkpoint
fn get_checkpoint(block_height: i32, is_testnet: bool) -> Option<(i32, &'static str)> {
    let test_network_checkpoins = [(1125777, "018c325f63f5cc98541cfef957f64845c86cf928e317ecc71a14debd364c7b8f57013c6f50deb5f788d5ac9105915ab9cbcda21a101d267c6424aa75b6e8df969e480d00016a2b0e3728a820b7982d81c87b80468ce65a4081843b890307115ca896416f3901e105bf42db29eca36e7235bd55546726753d1f967c3f284e243cbb3b3375d95a01a3ce8339e68a22d91b0750ef45468efe763e3d5e3a6e59809ddadcc94fe73c6c01494803bd8e6b730cb277701c613a4e7355cb54f79653724618e1436c02fca30c0177d25b5ed812af45eb46b54bc37c3fbe08fdfb4d952bb917fe59187bc78c42640001088b8a9fc4769017f3fdf865637e5cebbeaf7a4c643247723bf009da5eb1e4340001e877753448933a336fcf9399cc3dcd357344510c79db717e976979cb2eab612d0001cb846820acd916b4ea03b0a222b3eae8704bbd5365f105156041c578bd214c3201e03719b3810c7a9eaf6680ad3c60fb5ffdb0106975c952ef173c3e8cde943b03")];
    let main_network_checkpoins = [(0, "0")];
    let used_checkpoints = if is_testnet {
        test_network_checkpoins
    } else {
        main_network_checkpoins
    };
    return used_checkpoints
        .iter()
        .rev()
        .filter(|x| x.0 < block_height)
        .next()
        .copied();
}

//TODO: update once we have more checkpoints
#[cfg(test)]
mod test {
    use crate::checkpoint::get_checkpoint;
    #[test]
    fn check_testnet_checkpoints() {
        assert_eq!(get_checkpoint(11257770, true).unwrap().0, 1125777);
        assert_eq!(get_checkpoint(-1, true), None);
    }
    #[test]
    fn check_mainnet_checkpoints() {}
}
