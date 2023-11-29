import { v4 as genuuid } from "uuid";

interface PIVXShieldParams {
  seed?: number[];
  extendedSpendingKey?: string;
  extendedFullViewingKey?: string;
  blockHeight: number;
  coinType: number;
  accountIndex?: number;
  loadSaplingData?: boolean;
}

interface Block {
  txs: {
    hex: string;
    txid: string;
  }[];
  height: number;
}

interface TransactionResult {
  decrypted_notes: [Note, string][];
  commitment_tree: string;
  nullifiers: string[];
}

interface Transaction {
  address: string;
  amount: number;
  blockHeight: number;
  useShieldInputs: boolean;
  utxos: UTXO[];
  transparentChangeAddress: string;
}

interface CreateTransactionReturnValue {
  txid: string;
  txhex: string;
  nullifiers: string[];
}

export class PIVXShield {
  /**
   * Webassembly object that holds Shield related functions
   */
  private shieldWorker: Worker;

  /**
   * Extended spending key
   */
  private extsk?: string;

  /**
   * Extended full viewing key
   */
  private extfvk: string;

  /**
   * Diversifier index of the last generated address.
   */
  private diversifierIndex = new Array<number>(11).fill(0);

  isTestnet: boolean;

  /**
   * Last processed block in the blockchain
   */
  private lastProcessedBlock: number;

  /**
   * Hex encoded commitment tree
   */
  private commitmentTree: string;

  /**
   * Array of notes, corresponding witness
   * @private
   */
  private unspentNotes: [Note, string][] = [];

  /**
   * A map txid->nullifiers, storing pending transaction.
   */
  private pendingSpentNotes: Map<string, string[]> = new Map();

  /**
   * A map txid->Notes, storing incoming spendable notes.
   */
  private pendingUnspentNotes: Map<string, Note[]> = new Map();

  /**
   * Array in which own addresses are stored
   */
  private ownAddresses: string[] = [];

  private promises: Map<
    string,
    { res: (...args: any) => void; rej: (...args: any) => void }
  > = new Map();

  private initWorker() {
    this.shieldWorker.onmessage = (msg) => {
      const promise = this.promises.get(msg.data.uuid);
      if (!promise)
        throw new Error(
          "Internal error: promise is undefined. Report this to https://github.com/PIVX-Labs/pivx-shield",
        );
      const { res, rej } = promise;
      if (msg.data.rej) {
        rej(msg.data.rej);
      } else {
        res(msg.data.res);
      }
      this.promises.delete(msg.data.uuid);
    };
  }

  private async callWorker<T>(name: string, ...args: any[]): Promise<T> {
    const uuid = genuuid();
    return await new Promise<T>((res, rej) => {
      this.promises.set(uuid, { res, rej });
      this.shieldWorker.postMessage({ uuid, name, args });
    });
  }
  /**
   * Creates a PIVXShield object
   * @param o - options
   * @param o.seed - array of 32 bytes that represents a random seed.
   * @param o.extendedSpendingKey - Extended Spending Key.
   * @param o.extendedFullViewingKey - Full viewing key
   * @param o.blockHeight - number representing the block height of creation of the wallet
   * @param o.coinType - number representing the coin type, 1 represents testnet
   * @param o.accountIndex - index of the account that you want to generate, by default is set to 0
   * @param o.loadSaplingData - if you want to load sapling parameters on creation, by deafult is set to true
   */
  static async create({
    seed,
    extendedSpendingKey,
    extendedFullViewingKey,
    blockHeight,
    coinType,
    accountIndex = 0,
    loadSaplingData = true,
  }: PIVXShieldParams) {
    if (!extendedSpendingKey && !seed && !extendedFullViewingKey) {
      throw new Error(
        "At least one among seed, extendedSpendingKey, extendedFullViewingKey must be provided",
      );
    }

    if (extendedSpendingKey && seed) {
      throw new Error("Don't provide both a seed and an extendedSpendingKey");
    }

    const shieldWorker = new Worker(
      new URL("worker_start.js", import.meta.url),
    );
    await new Promise<void>((res) => {
      shieldWorker.onmessage = (msg) => {
        if (msg.data === "done") res();
      };
    });

    const isTestnet = coinType === 1;

    const pivxShield = new PIVXShield(
      shieldWorker,
      extendedSpendingKey,
      extendedFullViewingKey ?? "",
      isTestnet,
      0,
      "",
    );

    if (loadSaplingData) {
      await pivxShield.loadSaplingProver();
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
        isTestnet,
      );
    }

    const [effectiveHeight, commitmentTree] = await pivxShield.callWorker<
      [number, string]
    >("get_closest_checkpoint", blockHeight, isTestnet);
    pivxShield.lastProcessedBlock = effectiveHeight;
    pivxShield.commitmentTree = commitmentTree;

    return pivxShield;
  }
  private constructor(
    shieldWorker: Worker,
    extsk: string | undefined,
    extfvk: string,
    isTestnet: boolean,
    nHeight: number,
    commitmentTree: string,
  ) {
    this.shieldWorker = shieldWorker;
    this.extsk = extsk;
    this.extfvk = extfvk;
    this.isTestnet = isTestnet;
    this.lastProcessedBlock = nHeight;

    this.commitmentTree = commitmentTree;
    this.initWorker();
  }
  /**
   * Load an extended spending key in order to have spending authority
   * @param enc_extsk - extended spending key
   * @throws Error if the spending key doesn't match with the stored viewing key
   */
  async loadExtendedSpendingKey(enc_extsk: string) {
    if (this.extsk) {
      throw new Error("A spending key is aready loaded");
    }
    const enc_extfvk = await this.callWorker(
      "generate_extended_full_viewing_key",
      enc_extsk,
      this.isTestnet,
    );
    if (enc_extfvk !== this.extfvk) {
      throw new Error("Extended full viewing keys do not match");
    }
    this.extsk = enc_extsk;
  }

  /**
   * @returns a string that saves the public shield data.
   * The seed or extended spending key still needs to be provided
   * if spending authority is needed
   */
  save() {
    return JSON.stringify({
      extfvk: this.extfvk,
      lastProcessedBlock: this.lastProcessedBlock,
      commitmentTree: this.commitmentTree,
      diversifierIndex: this.diversifierIndex,
      unspentNotes: this.unspentNotes,
      isTestnet: this.isTestnet,
    });
  }
  /**
   * Creates a PIVXShield object from shieldData
   * @param data - output of save() function
   */
  static async load(data: string) {
    const shieldData = JSON.parse(data);
    const shieldWorker = new Worker(
      new URL("worker_start.js", import.meta.url),
    );

    await new Promise<void>((res) => {
      shieldWorker.onmessage = (msg) => {
        if (msg.data === "done") res();
      };
    });
    const pivxShield = new PIVXShield(
      shieldWorker,
      undefined,
      shieldData.extfvk,
      shieldData.isTestnet,
      shieldData.lastProcessedBlock,
      shieldData.commitmentTree,
    );
    pivxShield.diversifierIndex = shieldData.diversifierIndex;
    pivxShield.unspentNotes = shieldData.unspentNotes;
    await pivxShield.loadAddresses();
    return pivxShield;
  }

  /**
   * Loop through the txs of a block and update useful shield data
   * @param block - block outputted from any PIVX node
   */
  async handleBlock(block: Block) {
    if (this.lastProcessedBlock > block.height) {
      throw new Error(
        "Blocks must be processed in a monotonically increasing order!",
      );
    }
    for (const tx of block.txs) {
      await this.addTransaction(tx.hex);
      this.pendingUnspentNotes.delete(tx.txid);
    }
    this.lastProcessedBlock = block.height;
  }

  async addTransaction(hex: string, decryptOnly = false) {
    const res = await this.callWorker<TransactionResult>(
      "handle_transaction",
      this.commitmentTree,
      hex,
      this.extfvk,
      this.isTestnet,
      this.unspentNotes,
    );
    if (!decryptOnly) {
      this.commitmentTree = res.commitment_tree;
      this.unspentNotes = res.decrypted_notes;

      if (res.nullifiers.length > 0) {
        await this.removeSpentNotes(res.nullifiers);
      }
    }
    return res.decrypted_notes.filter(
      (note) =>
        !this.unspentNotes.some(
          (note2) => JSON.stringify(note2[0]) === JSON.stringify(note[0]),
        ),
    );
  }

  /**
   * Remove the Shield Notes that match the nullifiers given in input
   * @param nullifiers - Array of nullifiers
   */
  private async removeSpentNotes(nullifiers: string[]) {
    this.unspentNotes = await this.callWorker(
      "remove_spent_notes",
      this.unspentNotes,
      nullifiers,
      this.extfvk,
      this.isTestnet,
    );
  }
  /**
   * @returns number of shield satoshis of the account
   */
  getBalance() {
    return this.unspentNotes.reduce((acc, [note]) => acc + note.value, 0);
  }

  /**
   * @returns number of pending satoshis of the account
   */
  getPendingBalance() {
    return Array.from(this.pendingUnspentNotes.values())
      .flat()
      .reduce((acc, v) => acc + v.value, 0);
  }

  /**
   * Creates a transaction, sending `amount` satoshis to the address
   */
  async createTransaction({
    address,
    amount,
    blockHeight,
    useShieldInputs = true,
    utxos,
    transparentChangeAddress,
  }: Transaction) {
    if (!this.extsk) {
      throw new Error("You cannot create a transaction in view only mode!");
    }
    if (!useShieldInputs && !transparentChangeAddress) {
      throw new Error("Change must have the same type of input used!");
    }
    const { txid, txhex, nullifiers } =
      await this.callWorker<CreateTransactionReturnValue>(
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
          is_testnet: this.isTestnet,
        },
      );

    if (useShieldInputs) {
      this.pendingSpentNotes.set(txid, nullifiers);
    }
    this.pendingUnspentNotes.set(
      txid,
      (await this.addTransaction(txhex, true)).map((n) => n[0]),
    );
    return {
      hex: txhex,
      spentUTXOs: useShieldInputs
        ? []
        : nullifiers.map((u) => {
            const [txid, vout] = u.split(",");
            return { txid, vout: Number.parseInt(vout) };
          }),
      txid,
    };
  }
  /**
   * @returns a number from 0.0 to 1.0 rapresenting
   * the progress of the transaction proof. If multicore is unavailable,
   * it always returns 0.0
   */
  async getTxStatus() {
    return await this.callWorker<number>("read_tx_progress");
  }
  /**
   * Signals the class that a transaction was sent successfully
   * and the notes can be marked as spent
   * @param txid - Transaction id
   */
  async finalizeTransaction(txid: string) {
    const nullifiers = this.pendingSpentNotes.get(txid);
    await this.removeSpentNotes(nullifiers ?? []);
    this.pendingSpentNotes.delete(txid);
  }
  /**
   * Discards the transaction, for example if
   * there were errors in sending them.
   * The notes won't be marked as spent.
   * @param txid - Transaction id
   */
  discardTransaction(txid: string) {
    this.pendingSpentNotes.delete(txid);
    this.pendingUnspentNotes.delete(txid);
  }

  /**
   * @returns new shield address
   */
  async getNewAddress() {
    const { address, diversifier_index } = await this.callWorker<{
      address: string;
      diversifier_index: number[];
    }>(
      "generate_next_shielding_payment_address",
      this.extfvk,
      this.diversifierIndex,
      this.isTestnet,
    );
    this.diversifierIndex = diversifier_index;
    this.ownAddresses.push(address);
    return address;
  }

  /**
   * @param address_to_check - shield address
   * @returns true iff the shield address belongs to the wallet
   */
  isMyAddress(address_to_check: string) {
    return this.ownAddresses.includes(address_to_check);
  }

  /**
   * loads used addresses
   */
  async loadAddresses() {
    let currentDiversifierIndex = new Array<number>(11).fill(0);
    const totIterations = this.diversifierIndex.reduce(
      (s, n, i) => s + n * 256 ** i,
      0,
    );
    let j = 0;
    while (j <= totIterations) {
      const { address, diversifier_index } = await this.callWorker<{
        address: string;
        diversifier_index: number[];
      }>(
        "generate_next_shielding_payment_address",
        this.extfvk,
        currentDiversifierIndex,
        this.isTestnet,
      );
      currentDiversifierIndex = diversifier_index;
      j = currentDiversifierIndex.reduce((s, n, i) => s + n * 256 ** i, 0);
      this.ownAddresses.push(address);
    }
    return false;
  }

  /**
   * Load sapling prover. Must be done to create a transaction,
   * But will be done lazily if note called explicitally.
   * @returns resolves when the sapling prover is loaded
   */
  async loadSaplingProver() {
    return await this.callWorker<void>("load_prover");
  }

  /**
   * @returns The last block that has been decoded
   */
  getLastSyncedBlock() {
    return this.lastProcessedBlock;
  }
}

export interface UTXO {
  txid: string;
  vout: number;
  amount?: number;
  private_key?: Uint8Array;
  script?: Uint8Array;
}

export interface Note {
  recipient: number[];
  value: number;
  rseed: number[];
}

export interface ShieldData {
  extfvk: string;
  lastProcessedBlock: number;
  commitmentTree: string;
  diversifierIndex: Uint8Array;
  unspentNotes: [Note, string][];
  isTestnet: boolean;
}
