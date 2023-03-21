import bs58 from "bs58";
import { v4 as genuuid } from "uuid";

export class PIVXShielding {
  initWorker() {
    this.promises = new Map();
    this.shieldWorker.onmessage = (msg) => {
      this.promises.get(msg.data.uuid).res(msg.data.res);
      this.promises.delete(msg.data.uuid);
    };
  }

  async callWorker(name, ...args) {
    const uuid = genuuid();
    return await new Promise((res, rej) => {
      this.promises.set(uuid, { res, rej });
      this.shieldWorker.postMessage({ uuid, name, args });
    });
  }
  /**
   * Creates a PIVXShielding object
   * @param {Object} o - options
   * @param {Array<Number>?} o.seed - array of 32 bytes that represents a random seed.
   * @param {String?} o.extendedSpendingKey - Extended Spending Key.
   * @param {Number} o.blockHeight - number representing the block height of creation of the wallet
   * @param {Number} o.coinType - number representing the coin type, 1 represents testnet
   * @param {Number} o.accountIndex - index of the account that you want to generate, by default is set to 0
   * @param {Boolean} o.loadSaplingData - if you want to load sapling parameters on creation, by deafult is set to true
   */
  static async create({
    seed,
    extendedSpendingKey,
    blockHeight,
    coinType,
    accountIndex = 0,
    loadSaplingData = true,
  }) {
    if (!extendedSpendingKey && !seed) {
      throw new Error("One of seed or extendedSpendingKey must be provided");
    }

    const shieldWorker = new Worker(
      new URL("worker_start.js", import.meta.url)
    );
    await new Promise((res) => {
      shieldWorker.onmessage = (msg) => {
        if (msg.data === "done") res();
      };
    });

    const isTestNet = coinType == 1 ? true : false;

    const pivxShielding = new PIVXShielding(
      shieldWorker,
      extendedSpendingKey,
      isTestNet,
      null,
      null
    );

    if (loadSaplingData) {
      if (!(await pivxShielding.loadSaplingProver())) {
        throw new Error("Cannot load sapling data");
      }
    }
    if (!extendedSpendingKey) {
      const serData = {
        seed: seed,
        coin_type: coinType,
        account_index: accountIndex,
      };
      extendedSpendingKey = await pivxShielding.callWorker(
        "generate_extended_spending_key_from_seed",
        serData
      );
      pivxShielding.extsk = extendedSpendingKey;
    }

    const [effectiveHeight, commitmentTree] = await pivxShielding.callWorker(
      "get_closest_checkpoint",
      blockHeight,
      isTestNet
    );
    pivxShielding.lastProcessedBlock = effectiveHeight;
    pivxShielding.commitmentTree = commitmentTree;
    return pivxShielding;
  }

  constructor(shieldWorker, extsk, isTestNet, nHeight, commitmentTree) {
    /**
     * Webassembly object that holds Shield related functions
     * @private
     */
    this.shieldWorker = shieldWorker;
    /**
     * Extended spending key
     * @type {String}
     * @private
     */
    this.extsk = extsk;
    /**
     * Diversifier index of the last generated address
     * @type {Uint8Array}
     * @private
     */
    this.diversifierIndex = new Uint8Array(11);
    /**
     * @type {Boolean}
     * @private
     */
    this.isTestNet = isTestNet;
    /**
     * Last processed block in the blockchain
     * @type {Number}
     * @private
     */
    this.lastProcessedBlock = nHeight;
    /**
     * Hex encoded commitment tree
     * @type {String}
     * @private
     */
    this.commitmentTree = commitmentTree;
    /**
     * Array of notes, corresponding witness
     * @type {[Note, String][]}
     * @private
     */
    this.unspentNotes = [];

    /**
     * @type {Map<String, String[]>} A map txid->nullifiers, storing pending transaction.
     * @private
     */

    this.pendingSpentNotes = new Map();

    this.initWorker();
  }

  /**
   * Loop through the txs of a block and update useful shield data
   * @param {{txs: String[], height: Number}} blockJson - Json of the block outputted from any PIVX node
   */
  async handleBlock(blockJson) {
    if (this.lastProcessedBlock > blockJson.height) {
      throw new Error(
        "Blocks must be processed in a monotonically increasing order!"
      );
    }
    for (const tx of blockJson.txs) {
      await this.addTransaction(tx.hex);
    }
    this.lastProcessedBlock = blockJson.height;
  }
  /**
   * Adds a transaction to the tree. Decrypts notes and stores nullifiers
   * @param {String} hex - transaction hex
   */
  async addTransaction(hex) {
    const res = await this.callWorker(
      "handle_transaction",
      this.commitmentTree,
      hex,
      this.extsk,
      this.isTestNet,
      this.unspentNotes
    );
    this.commitmentTree = res.commitment_tree;
    this.unspentNotes = res.decrypted_notes;

    if (res.nullifiers.length > 0) {
      await this.removeSpentNotes(res.nullifiers);
    }
  }

  /**
   * Remove the Shield Notes that match the nullifiers given in input
   * @param {Array<String>} blockJson - Array of nullifiers
   */
  async removeSpentNotes(nullifiers) {
    this.unspentNotes = await this.callWorker(
      "remove_spent_notes",
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
    return this.unspentNotes.reduce((acc, [note]) => acc + note.value, 0);
  }

  /**
   * Creates a transaction, sending `amount` satoshis to the address
   * @param {{address: String, amount: Number, blockHeight: Number, useShieldInputs: bool, utxos: UTXO[]?}} target
   * @returns {{hex: String, spentUTXOs: UTXO[]}}
   */
  async createTransaction({
    address,
    amount,
    blockHeight,
    useShieldInputs = true,
    utxos,
  }) {
    const { txid, txhex, nullifiers } = await this.callWorker(
      "create_transaction",
      {
        notes: useShieldInputs ? this.unspentNotes : null,
        utxos: useShieldInputs ? null : utxos,
        extsk: this.extsk,
        to_address: address,
        change_address: await this.getNewAddress(),
        amount,
        block_height: blockHeight,
        is_testnet: this.isTestNet,
      }
    );

    if (useShieldInputs) {
      this.pendingSpentNotes.set(txid, nullifiers);
    }
    return {
      hex: txhex,
      spentUTXOs: useShieldInputs
        ? []
        : nullifiers.map((u) => {
            const [txid, vout] = u.split(",");
            return new UTXO({ txid, vout: Number.parseInt(vout) });
          }),
    };
  }

  /**
   * Signals the class that a transaction was sent successfully
   * and the notes can be marked as spent
   * @throws Error if txid is not found
   * @param{String} txid - Transaction id
   */
  async finalizeTransaction(txid) {
    const nullifiers = this.pendingSpentNotes.get(txid);
    await this.removeSpentNotes(nullifiers);
    this.discardTransaction(txid);
  }
  /**
   * Discards the transaction, for example if
   * there were errors in sending them.
   * The notes won't be marked as spent.
   * @param{String} txid - Transaction id
   */
  discardTransaction(txid) {
    this.pendingSpentNotes.clear(txid);
  }

  /**
   * @returns {String} new shielded address
   */
  async getNewAddress() {
    const { address, diversifier_index } = await this.callWorker(
      "generate_next_shielding_payment_address",
        this.extsk,
        this.diversifierIndex,
        this.isTestNet
      );
    this.diversifierIndex = diversifier_index;
    return address;
  }

  async loadSaplingProver() {
    return await this.callWorker("load_prover");
  }

  /**
   * @returns {Number} The last block that has been decoded
   */
  getLastSyncedBlock() {
    return this.lastProcessedBlock;
  }
}

export class Note {
  /**
   * Class corresponding to an unspent sapling shield note
   * @param {Array<Number>} o.recipient - Recipient PaymentAddress encoded as a byte array
   * @param {Number} o.value - How much PIVs are in the note
   * @param {Array<Number>} o.rseed - Random seed encoded as a byte array
   */
  constructor({ recipient, value, rseed }) {
    this.recipient = recipient;
    this.value = value;
    this.rseed = rseed;
  }
}

export class UTXO {
  /**
   * Add a transparent UTXO, along with its private key
   * @param {Object} o - Options
   * @param {String} o.txid - Transaction ID of the UTXO
   * @param {Number} o.vout - output index of the UTXO
   * @param {Number?} o.amount - Value in satoshi of the UTXO
   * @param {String?} o.privateKey - Private key associated to the UTXO
   * @param {Uint8Array?} o.script - Tx Script
   */
  constructor({ txid, vout, amount, privateKey, script }) {
    this.txid = txid;
    this.vout = vout;
    this.amount = amount;
    this.private_key = privateKey ? bs58.decode(privateKey).slice(1, 33) : null;
    this.script = script;
  }
}
