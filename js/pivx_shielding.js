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
    this.unspentNotes = [];
  }

  /**
   * Loop through the txs of a block and update useful shield data
   * @param {JSON} blockJson - Json of the block outputted from any PIVX node
   */
  handleBlock(blockJson) {
    for (let tx of blockJson.tx) {
      this.addTransaction(tx.hex);
    }
  }
  /**
   * Adds a transaction to the tree. Decrypts notes and stores nullifiers
   * @param {String} hex - transaction hex
   */
  addTransaction(hex) {
    let res = this.shieldMan.handle_transaction(
      this.commitmentTree,
      hex,
      this.extsk,
      this.isTestNet
    );
    this.commitmentTree = res.commitment_tree;
    for (let x of res.decrypted_notes) {
      this.unspentNotes.push(x);
    }
    if (res.nullifiers.length > 0) {
      this.removeSpentNotes(res.nullifiers);
    }
  }

  /**
   * Remove the Shield Notes that match the nullifiers given in input
   * @param {Array<String>} blockJson - Array of nullifiers
   */
  removeSpentNotes(nullifiers) {
    this.unspentNotes = this.shieldMan.remove_unspent_notes(
      this.unspentNotes,
      nullifiers,
      this.extsk,
      this.isTestNet
    );
  }
  /**
   * Return number of shielded satoshis of the account
   */
  getBalance() {
    let tot = 0;
    for (let x of this.unspentNotes) {
      tot += x[0].value;
    }
    return tot;
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
