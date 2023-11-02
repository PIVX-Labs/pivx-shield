import bs58 from "bs58";
import { v4 as genuuid } from "uuid";

export class PIVXShield {
  initWorker() {
    this.promises = new Map();
    this.shieldWorker.onmessage = (msg) => {
      const { res, rej } = this.promises.get(msg.data.uuid);
      if (msg.data.rej) {
        rej(msg.data.rej);
      } else {
        res(msg.data.res);
      }
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
   * Creates a PIVXShield object
   * @param {Object} o - options
   * @param {String?} o.data - ShieldData string in JSON format.
   * @param {Array<Number>?} o.seed - array of 32 bytes that represents a random seed.
   * @param {String?} o.extendedSpendingKey - Extended Spending Key.
   * @param {String?} o.extendedFullViewingKey - Full viewing key
   * @param {Number} o.blockHeight - number representing the block height of creation of the wallet
   * @param {Number} o.coinType - number representing the coin type, 1 represents testnet
   * @param {Number} o.accountIndex - index of the account that you want to generate, by default is set to 0
   * @param {Boolean} o.loadSaplingData - if you want to load sapling parameters on creation, by deafult is set to true
   */
  static async create({
    data,
    seed,
    extendedSpendingKey,
    extendedFullViewingKey,
    blockHeight,
    coinType,
    accountIndex = 0,
    loadSaplingData = true,
  }) {
    if (!extendedSpendingKey && !seed && !extendedFullViewingKey && !data) {
      throw new Error(
        "At least one among seed, extendedSpendingKey, extendedFullViewingKey or backup data must be provided",
      );
    }

    if (extendedSpendingKey && seed) {
      throw new Error("Don't provide both a seed and an extendedSpendingKey");
    }

    const shieldWorker = new Worker(
      new URL("worker_start.js", import.meta.url),
    );
    await new Promise((res) => {
      shieldWorker.onmessage = (msg) => {
        if (msg.data === "done") res();
      };
    });

    const isTestNet = coinType == 1 ? true : false;

    const pivxShield = new PIVXShield(
      shieldWorker,
      extendedSpendingKey,
      extendedFullViewingKey,
      isTestNet,
      null,
      null,
    );

    if (loadSaplingData) {
      if (!(await pivxShield.loadSaplingProver())) {
        throw new Error("Cannot load sapling data");
      }
    }
    if (seed) {
      const serData = {
        seed: seed,
        coin_type: coinType,
        account_index: accountIndex,
      };
      extendedSpendingKey = await pivxShield.callWorker(
        "generate_extended_spending_key_from_seed",
        serData,
      );
      pivxShield.extsk = extendedSpendingKey;
    }
    if (extendedSpendingKey) {
      pivxShield.extfvk = await pivxShield.callWorker(
        "generate_extended_full_viewing_key",
        pivxShield.extsk,
        isTestNet,
      );
    }
    let readFromData = false;
    if (data) {
      const shieldData = JSON.parse(data);
      if (pivxShield.load(shieldData)) {
        readFromData = true;
      }
    }
    if (!readFromData) {
      const [effectiveHeight, commitmentTree] = await pivxShield.callWorker(
        "get_closest_checkpoint",
        blockHeight,
        isTestNet,
      );
      pivxShield.lastProcessedBlock = effectiveHeight;
      pivxShield.commitmentTree = commitmentTree;
    }
    return pivxShield;
  }

  constructor(shieldWorker, extsk, extfvk, isTestNet, nHeight, commitmentTree) {
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
     * Extended full viewing key
     * @type {String}
     * @private
     */
    this.extfvk = extfvk;
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

    /**
     * @type {Map<String, Note[]>} A map txid->Notes, storing incoming spendable notes.
     * @private
     */
    this.pendingUnspentNotes = new Map();

    this.initWorker();
  }

  /**
   * Load an extended spending key in order to have spending authority
   * @param {String} enc_extsk - extended spending key
   */
  async loadExtendedSpendingKey(enc_extsk) {
    if (this.extsk) {
      throw new Error("A spending key is aready loaded");
    }
    const enc_extfvk = await this.callWorker(
      "generate_extended_full_viewing_key",
      enc_extsk,
      this.isTestNet,
    );
    if (enc_extfvk !== this.extfvk) {
      throw new Error("Extended full viewing keys do not match");
    }
    this.extsk = enc_extsk;
  }

  //Save your shield data
  save() {
    return JSON.stringify(
      new ShieldData({
        extfvk: this.extfvk,
        lastProcessedBlock: this.lastProcessedBlock,
        commitmentTree: this.commitmentTree,
        diversifierIndex: this.diversifierIndex,
        unspentNotes: this.unspentNotes,
      }),
    );
  }

  /**
   * Load shieldWorker from a shieldData
   * @param {ShieldData} shieldData - shield data
   */
  load(shieldData) {
    if (this.extfvk && this.extfvk != shieldData.extfvk) {
      return false;
    }
    this.extfvk = shieldData.extfvk;
    this.commitmentTree = shieldData.commitmentTree;
    this.unspentNotes = shieldData.unspentNotes;
    this.lastProcessedBlock = shieldData.lastProcessedBlock;
    this.diversifierIndex = shieldData.diversifierIndex;
    return true;
  }
  /**
   * Loop through the txs of a block and update useful shield data
   * @param {{txs: String[], height: Number}} blockJson - Json of the block outputted from any PIVX node
   */
  async handleBlock(blockJson) {
    if (this.lastProcessedBlock > blockJson.height) {
      throw new Error(
        "Blocks must be processed in a monotonically increasing order!",
      );
    }
    for (const tx of blockJson.txs) {
      await this.addTransaction(tx.hex);
      this.pendingUnspentNotes.delete(tx.txid);
    }
    this.lastProcessedBlock = blockJson.height;
  }
  /**
   * Adds a transaction to the tree. Decrypts notes and stores nullifiers
   * @param {String} hex - transaction hex
   */
  async addTransaction(hex, decryptOnly = false) {
    const res = await this.callWorker(
      "handle_transaction",
      this.commitmentTree,
      hex,
      this.extfvk,
      this.isTestNet,
      this.unspentNotes,
    );
    if (decryptOnly) {
      return res.decrypted_notes.filter(
        (note) =>
          !this.unspentNotes.some(
            (note2) => JSON.stringify(note2[0]) === JSON.stringify(note[0]),
          ),
      );
    } else {
      this.commitmentTree = res.commitment_tree;
      this.unspentNotes = res.decrypted_notes;

      if (res.nullifiers.length > 0) {
        await this.removeSpentNotes(res.nullifiers);
      }
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
      this.extfvk,
      this.isTestNet,
    );
  }
  /**
   * Return number of shield satoshis of the account
   */
  getBalance() {
    return this.unspentNotes.reduce((acc, [note]) => acc + note.value, 0);
  }

  /**
   * Return number of pending satoshis of the account
   */
  getPendingBalance() {
    return Array.from(this.pendingUnspentNotes.values())
      .flat()
      .reduce((acc, v) => acc + v[0].value, 0);
  }

  /**
   * Creates a transaction, sending `amount` satoshis to the address
   * @param {{address: String, amount: Number, blockHeight: Number, useShieldInputs: bool, utxos: UTXO[]?, transparentChangeAddress: String?}} target
   * @returns {{hex: String, spentUTXOs: UTXO[]}}
   */
  async createTransaction({
    address,
    amount,
    blockHeight,
    useShieldInputs = true,
    utxos,
    transparentChangeAddress,
  }) {
    if (!this.extsk) {
      throw new Error("You cannot create a transaction in view only mode!");
    }
    if (!useShieldInputs && !transparentChangeAddress) {
      throw new Error("Change must have the same type of input used!");
    }
    const { txid, txhex, nullifiers } = await this.callWorker(
      "create_transaction",
      {
        notes: useShieldInputs ? this.unspentNotes : null,
        utxos: useShieldInputs ? null : utxos,
        extsk: this.extsk,
        to_address: address,
        change_address: useShieldInputs
          ? await this.getNewAddress()
          : transparentChangeAddress,
        amount,
        block_height: blockHeight,
        is_testnet: this.isTestNet,
      },
    );

    if (useShieldInputs) {
      this.pendingSpentNotes.set(txid, nullifiers);
    }
    this.pendingUnspentNotes.set(txid, await this.addTransaction(txhex, true));
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
  async getTxStatus() {
    return await this.callWorker("read_tx_progress");
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
    this.pendingSpentNotes.delete(txid);
  }
  /**
   * Discards the transaction, for example if
   * there were errors in sending them.
   * The notes won't be marked as spent.
   * @param{String} txid - Transaction id
   */
  discardTransaction(txid) {
    this.pendingSpentNotes.delete(txid);
    this.pendingUnspentNotes.delete(txid);
  }

  /**
   * @returns {String} new shield address
   */
  async getNewAddress() {
    const { address, diversifier_index } = await this.callWorker(
      "generate_next_shielding_payment_address",
      this.extfvk,
      this.diversifierIndex,
      this.isTestNet,
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

class ShieldData {
  /**
   * Add a transparent UTXO, along with its private key
   * @param {Object} o - Options
   * @param {String} o.extfvk - Extended full viewing key
   * @param {Number} o.lastProcessedBlock - Last processed block in blockchain
   * @param {String} o.commitmentTree - Hex encoded commitment tree
   * @param {Uint8Array} o.diversifierIndex - Diversifier index of the last generated address
   * @param {[Note, String][]} o.unspentNotes - Array of notes, corresponding witness
   */
  constructor({
    extfvk,
    lastProcessedBlock,
    commitmentTree,
    diversifierIndex,
    unspentNotes,
  }) {
    this.extfvk = extfvk;
    this.diversifierIndex = diversifierIndex;
    this.lastProcessedBlock = lastProcessedBlock;
    this.commitmentTree = commitmentTree;
    this.unspentNotes = unspentNotes;
  }
}
