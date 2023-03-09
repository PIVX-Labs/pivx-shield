export default class PIVXShielding {
  /**
   * Creates a PIVXShielding object
   * @param {Array<Number>} seed - array of 32 bytes that represents a random seed
   * @param {Number} blockHeight - number representing the block height of creation of the wallet
   * @param {Number} coinType - number representing the coin type, 1 represents testnet
   * @param {Number} accountIndex - index of the account that you want to generate, by default is set to 0
   */
  static async createFromSeed(seed, blockHeight, coinType, accountIndex = 0) {
    const shieldMan = await import("pivx-shielding");
    const serData = {
      seed: seed,
      coin_type: coinType,
      account_index: accountIndex,
    };
    const extsk = shieldMan.generate_extended_spending_key_from_seed(serData);
    const isTestNet = coinType == 1 ? true : false;
    const checkpointResult = shieldMan.get_closest_checkpoint(
      blockHeight,
      isTestNet
    );
    const effectiveHeight = checkpointResult[0];
    const commitmentTree = checkpointResult[1];
    return new PIVXShielding(
      shieldMan,
      extsk,
      isTestNet,
      effectiveHeight,
      commitmentTree
    );
  }

  constructor(shieldMan, extsk, isTestNet, blockHeight, commitmentTree) {
    this.shieldMan = shieldMan;
    this.extsk = extsk;
    this.generatedAddresses = 0;
    this.isTestNet = isTestNet;
    this.lastBlock = blockHeight;
    this.commitmentTree = commitmentTree;
  }

  /**
   * Adds a transaction to the tree. Decrypts notes and stores nullifiers
   * @param {String} hex - transaction hex
   */
  addTransaction(hex) { //TODO: STILL A TEST
    let test_tree =
      "018c325f63f5cc98541cfef957f64845c86cf928e317ecc71a14debd364c7b8f57013c6f50deb5f788d5ac9105915ab9cbcda21a101d267c6424aa75b6e8df969e480d00016a2b0e3728a820b7982d81c87b80468ce65a4081843b890307115ca896416f3901e105bf42db29eca36e7235bd55546726753d1f967c3f284e243cbb3b3375d95a01a3ce8339e68a22d91b0750ef45468efe763e3d5e3a6e59809ddadcc94fe73c6c01494803bd8e6b730cb277701c613a4e7355cb54f79653724618e1436c02fca30c0177d25b5ed812af45eb46b54bc37c3fbe08fdfb4d952bb917fe59187bc78c42640001088b8a9fc4769017f3fdf865637e5cebbeaf7a4c643247723bf009da5eb1e4340001e877753448933a336fcf9399cc3dcd357344510c79db717e976979cb2eab612d0001cb846820acd916b4ea03b0a222b3eae8704bbd5365f105156041c578bd214c3201e03719b3810c7a9eaf6680ad3c60fb5ffdb0106975c952ef173c3e8cde943b03";
    let test_tx = "blob";
    let res = this.shieldMan.handle_transaction(
      test_tree,
      test_tx,
      this.extsk,
      this.isTestNet
    );
    console.log(res);
  }

  /**
   * Return number of shielded satoshis of the account
   */
  getBalance() {
    throw new Error("Not implemented");
  }

  /**
   * Creates a transaction, sending `amount` satoshis to the addresses
   * @param {{address: String, amount: String}[]} targets
   */
  createTransaction(targets) {
    throw new Error("Not implemented");
  }

  /**
   * @returns {string} new shielded address
   */
  getNewAddress() {
    const address = this.shieldMan.generate_next_shielding_payment_address(
      this.extsk,
      this.generatedAddresses + 1,
      this.isTestNet
    );
    this.generatedAddresses += 1;
    return address;
  }
}
