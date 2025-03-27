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

/**
 * Block that's deserialized in rust
 */
interface RustBlock {
  txs: string[];
}

interface TransactionResult {
  decrypted_notes: SpendableNote[];
  decrypted_new_notes: SpendableNote[];
  commitment_tree: string;
  nullifiers: string[];
  /**
   * hex of the transactions belonging to the wallet
   * i.e. either the spend or output belongs to us
   */
  wallet_transactions: string[];
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
   * integer to keep track of the current Shield version.
   * v1: added mapNullifierNote
   */
  static version = 1;

  /**
   * Webassembly object that holds Shield related functions
   */
  private static shieldWorker: Worker;

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
   * Array of spendable notes
   * @private
   */
  private unspentNotes: SpendableNote[] = [];

  /**
   * A map txid->nullifiers, storing pending transaction.
   */
  private pendingSpentNotes: Map<string, string[]> = new Map();

  /**
   * A map txid->Notes, storing incoming spendable notes.
   */
  private pendingUnspentNotes: Map<string, Note[]> = new Map();

  /**
   *
   * @private
   * Map nullifier -> Note
   * It contains all notes in the history of the wallet, both spent and unspent
   */
  private mapNullifierNote: Map<string, SimplifiedNote> = new Map();

  private static promises: Map<
    string,
    { res: (...args: any) => void; rej: (...args: any) => void }
  > = new Map();

  private static isInit = false;

  private static initWorker() {
    if (!PIVXShield.isInit) {
      PIVXShield.isInit = true;
      PIVXShield.shieldWorker.onmessage = (msg) => {
        if (!msg.data.uuid) return;
        const promise = PIVXShield.promises.get(msg.data.uuid);
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
        PIVXShield.promises.delete(msg.data.uuid);
      };
    }
  }

  private static async callWorker<T>(name: string, ...args: any[]): Promise<T> {
    const uuid = genuuid();
    return await new Promise<T>((res, rej) => {
      PIVXShield.promises.set(uuid, { res, rej });
      PIVXShield.shieldWorker.postMessage({ uuid, name, args });
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

    if (!PIVXShield.shieldWorker) {
      PIVXShield.shieldWorker = new Worker(
        new URL("worker_start.js", import.meta.url),
      );
      await new Promise<void>((res) => {
        PIVXShield.shieldWorker.onmessage = (msg) => {
          if (msg.data === "done") res();
        };
      });
    }

    const isTestnet = coinType === 1;

    const pivxShield = new PIVXShield(
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
      extendedSpendingKey = await PIVXShield.callWorker(
        "generate_extended_spending_key_from_seed",
        serData,
      );
      pivxShield.extsk = extendedSpendingKey;
    }
    if (extendedSpendingKey) {
      pivxShield.extfvk = await PIVXShield.callWorker(
        "generate_extended_full_viewing_key",
        pivxShield.extsk,
        isTestnet,
      );
    }

    const [effectiveHeight, commitmentTree] = await PIVXShield.callWorker<
      [number, string]
    >("get_closest_checkpoint", blockHeight, isTestnet);
    pivxShield.lastProcessedBlock = effectiveHeight;
    pivxShield.commitmentTree = commitmentTree;

    return pivxShield;
  }
  private constructor(
    extsk: string | undefined,
    extfvk: string,
    isTestnet: boolean,
    nHeight: number,
    commitmentTree: string,
  ) {
    this.extsk = extsk;
    this.extfvk = extfvk;
    this.isTestnet = isTestnet;
    this.lastProcessedBlock = nHeight;

    this.commitmentTree = commitmentTree;
    PIVXShield.initWorker();
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
    const enc_extfvk = await PIVXShield.callWorker(
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
   * Generate a spending key from the seed and load it using `PIVXShield::loadExtendedSpendingKey`
   * @throws Error if the generated spending key doesn't match the stored viewing key
   * or if a spending key is already loaded
   */
  async loadSeed(seed: Uint8Array, coinType: number, accountIndex: number) {
    const extsk = await PIVXShield.callWorker<string>(
      "generate_extended_spending_key_from_seed",
      {
        seed,
        coin_type: coinType,
        account_index: accountIndex,
      },
    );
    return await this.loadExtendedSpendingKey(extsk);
  }

  /**
   * @returns a string that saves the public shield data.
   * The seed or extended spending key still needs to be provided
   * if spending authority is needed
   */
  save() {
    return JSON.stringify({
      version: PIVXShield.version,
      extfvk: this.extfvk,
      lastProcessedBlock: this.lastProcessedBlock,
      commitmentTree: this.commitmentTree,
      diversifierIndex: this.diversifierIndex,
      unspentNotes: this.unspentNotes,
      isTestnet: this.isTestnet,
      mapNullifierNote: Object.fromEntries(this.mapNullifierNote),
    });
  }
  /**
   * Creates a PIVXShield object from shieldData
   * @param data - output of save() function
   */
  static async load(data: string) {
    const shieldData = JSON.parse(data);
    if (!PIVXShield.shieldWorker) {
      PIVXShield.shieldWorker = new Worker(
        new URL("worker_start.js", import.meta.url),
      );

      await new Promise<void>((res) => {
        PIVXShield.shieldWorker.onmessage = (msg) => {
          if (msg.data === "done") res();
        };
      });
    }
    const currVersion = shieldData.version ?? 0;
    const pivxShield = new PIVXShield(
      undefined,
      shieldData.extfvk,
      shieldData.isTestnet,
      shieldData.lastProcessedBlock,
      shieldData.commitmentTree,
    );

    if (currVersion >= 1) {
      pivxShield.mapNullifierNote = new Map(
        Object.entries(shieldData.mapNullifierNote),
      );
      pivxShield.unspentNotes = shieldData.unspentNotes;
    }
    pivxShield.diversifierIndex = shieldData.diversifierIndex;

    return { pivxShield, success: currVersion == PIVXShield.version };
  }

  async handleBlocks(blocks: Block[]) {
    if (blocks.length === 0) return [];
    if (
      !blocks.every((block, i) => {
        if (i === 0) {
          return block.height > this.lastProcessedBlock;
        } else {
          return block.height > blocks[i - 1].height;
        }
      })
    ) {
      throw new Error(
        "Blocks must be provided in monotonically increaisng order",
      );
    }

    for (const block of blocks) {
      for (const tx of block.txs) {
        this.pendingUnspentNotes.delete(tx.txid);
      }
    }

    const {
      decrypted_notes,
      decrypted_new_notes,
      nullifiers,
      commitment_tree,
      wallet_transactions,
    } = await PIVXShield.callWorker<TransactionResult>(
      "handle_blocks",
      this.commitmentTree,
      blocks.map((block) => {
        return {
          txs: block.txs.map(({ hex }) => hex),
        };
      }) satisfies RustBlock[],
      this.extfvk,
      this.isTestnet,
      this.unspentNotes,
    );
    this.commitmentTree = commitment_tree;
    this.unspentNotes = [...decrypted_notes, ...decrypted_new_notes];
    for (const { note, nullifier } of decrypted_new_notes) {
      const simplifiedNote = {
        value: note.value,
        recipient: await this.getShieldAddressFromNote(note),
      };

      this.mapNullifierNote.set(nullifier, simplifiedNote);
    }
    await this.removeSpentNotes(nullifiers);
    this.lastProcessedBlock = blocks[blocks.length - 1].height;

    return wallet_transactions;
  }

  /**
   * Loop through the txs of a block and update useful shield data
   * @param block - block outputted from any PIVX node
   * @returns list of transactions belonging to the wallet
   */
  async handleBlock(block: Block) {
    return await this.handleBlocks([block]);
  }

  /**
   *
   * @param note - Note and its corresponding witness
   * Generate the nullifier for a given pair note, witness
   */
  private async generateNullifierFromNote(note: [Note, String]) {
    return await PIVXShield.callWorker<string>(
      "get_nullifier_from_note",
      note,
      this.extfvk,
      this.isTestnet,
    );
  }

  private async getShieldAddressFromNote(note: Note) {
    return await PIVXShield.callWorker<string>(
      "encode_payment_address",
      this.isTestnet,
      note.recipient,
    );
  }
  async decryptTransactionOutputs(hex: string) {
    const decryptedNotes = await this.decryptTransaction(hex);
    const simplifiedNotes = [];
    for (const { note } of decryptedNotes) {
      simplifiedNotes.push({
        value: note.value,
        recipient: await this.getShieldAddressFromNote(note),
      });
    }
    return simplifiedNotes;
  }

  async decryptTransaction(hex: string) {
    const res = await PIVXShield.callWorker<TransactionResult>(
      "handle_blocks",
      this.commitmentTree,
      [{ txs: [hex] }] satisfies RustBlock[],
      this.extfvk,
      this.isTestnet,
      [],
    );
    return res.decrypted_new_notes;
  }

  /**
   * Remove the Shield Notes that match the nullifiers given in input
   * @param nullifiers - Array of nullifiers
   */
  private async removeSpentNotes(nullifiers: string[]) {
    for (let nullifier of nullifiers) {
      let i = this.unspentNotes.findIndex(
        (uNote) => uNote.nullifier === nullifier,
      );
      if (i !== -1) {
        this.unspentNotes.splice(i, 1);
      }
    }
    return;
  }
  /**
   * @returns number of shield satoshis of the account
   */
  getBalance() {
    return this.unspentNotes.reduce((acc, { note }) => acc + note.value, 0);
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
      await PIVXShield.callWorker<CreateTransactionReturnValue>(
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
    const decryptedNotes = await this.decryptTransaction(txhex);
    this.pendingUnspentNotes.set(
      txid,
      decryptedNotes.map(({ note }) => note),
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
    return await PIVXShield.callWorker<number>("read_tx_progress");
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
    const { address, diversifier_index } = await PIVXShield.callWorker<{
      address: string;
      diversifier_index: number[];
    }>(
      "generate_next_shielding_payment_address",
      this.extfvk,
      this.diversifierIndex,
      this.isTestnet,
    );
    this.diversifierIndex = diversifier_index;
    return address;
  }

  /**
   * Load sapling prover. Must be done to create a transaction,
   * But will be done lazily if note called explicitally.
   * @returns resolves when the sapling prover is loaded
   */
  async loadSaplingProver(url?: string) {
    if (url) {
      return await PIVXShield.callWorker<boolean>("load_prover_with_url", url);
    } else {
      return await PIVXShield.callWorker<boolean>("load_prover");
    }
  }

  async proverIsLoaded() {
    return await PIVXShield.callWorker<boolean>("prover_is_loaded");
  }

  async loadSaplingProverWithBytes(
    sapling_output_bytes: Uint8Array,
    sapling_spend_bytes: Uint8Array,
  ) {
    return await PIVXShield.callWorker<boolean>(
      "load_prover_with_bytes",
      sapling_output_bytes,
      sapling_spend_bytes,
    );
  }

  /**
   * @returns The last block that has been decoded
   */
  getLastSyncedBlock() {
    return this.lastProcessedBlock;
  }

  /**
   * @param nullifier - A sapling nullifier
   * @returns the Note corresponding to a given nullifier
   */
  getNoteFromNullifier(nullifier: string) {
    return this.mapNullifierNote.get(nullifier);
  }
  /**
   * @returns sapling root
   */
  async getSaplingRoot(): Promise<string> {
    return await PIVXShield.callWorker<string>(
      "get_sapling_root",
      this.commitmentTree,
    );
  }

  /**
   * Reloads from checkpoint. Needs to be resynced to use
   */
  async reloadFromCheckpoint(checkpointBlock: number): Promise<void> {
    const [effectiveHeight, commitmentTree] = await PIVXShield.callWorker<
      [number, string]
    >("get_closest_checkpoint", checkpointBlock, this.isTestnet);
    this.commitmentTree = commitmentTree;
    this.lastProcessedBlock = effectiveHeight;
    this.unspentNotes = [];
    this.pendingSpentNotes = new Map();
    this.pendingUnspentNotes = new Map();
    this.mapNullifierNote = new Map();
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

export interface SpendableNote {
  note: Note;
  witness: string;
  nullifier: string;
}

export interface SimplifiedNote {
  recipient: string;
  value: number;
}

export interface ShieldData {
  extfvk: string;
  lastProcessedBlock: number;
  commitmentTree: string;
  diversifierIndex: Uint8Array;
  unspentNotes: SpendableNote[];
  isTestnet: boolean;
}
