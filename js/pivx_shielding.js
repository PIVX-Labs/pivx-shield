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
    return new PIVXShielding(shieldMan, extsk, isTestNet);
  }

  constructor(shieldMan, extsk, isTestNet) {
    this.shieldMan = shieldMan;
    this.extsk = extsk;
    this.generatedAddresses = 0;
    this.isTestNet = isTestNet;
  }

  /**
   * Adds a transaction to the tree. Decrypts notes and stores nullifiers
   * @param {String} hex - transaction hex
   */
  addTransaction(hex) {
    throw new Error("Not implemented");
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
