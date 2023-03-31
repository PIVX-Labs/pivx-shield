export class PIVXShield {
    /**
     * Creates a PIVXShield object
     * @param {Object} o - options
     * @param {String?} o.data - ShieldData string in JSON format.
     * @param {Array<Number>?} o.seed - array of 32 bytes that represents a random seed.
     * @param {String?} o.extendedSpendingKey - Extended Spending Key.
     * @param {Number} o.blockHeight - number representing the block height of creation of the wallet
     * @param {Number} o.coinType - number representing the coin type, 1 represents testnet
     * @param {Number} o.accountIndex - index of the account that you want to generate, by default is set to 0
     * @param {Boolean} o.loadSaplingData - if you want to load sapling parameters on creation, by deafult is set to true
     */
    static create({ data, seed, extendedSpendingKey, blockHeight, coinType, accountIndex, loadSaplingData, }: {
        data: string | null;
        seed: Array<number> | null;
        extendedSpendingKey: string | null;
        blockHeight: number;
        coinType: number;
        accountIndex: number;
        loadSaplingData: boolean;
    }): Promise<PIVXShield>;
    constructor(shieldWorker: any, extsk: any, isTestNet: any, nHeight: any, commitmentTree: any);
    initWorker(): void;
    promises: any;
    callWorker(name: any, ...args: any[]): Promise<any>;
    /**
     * Webassembly object that holds Shield related functions
     * @private
     */
    private shieldWorker;
    /**
     * Extended spending key
     * @type {String}
     * @private
     */
    private extsk;
    /**
     * Diversifier index of the last generated address
     * @type {Uint8Array}
     * @private
     */
    private diversifierIndex;
    /**
     * @type {Boolean}
     * @private
     */
    private isTestNet;
    /**
     * Last processed block in the blockchain
     * @type {Number}
     * @private
     */
    private lastProcessedBlock;
    /**
     * Hex encoded commitment tree
     * @type {String}
     * @private
     */
    private commitmentTree;
    /**
     * Array of notes, corresponding witness
     * @type {[Note, String][]}
     * @private
     */
    private unspentNotes;
    /**
     * @type {Map<String, String[]>} A map txid->nullifiers, storing pending transaction.
     * @private
     */
    private pendingSpentNotes;
    /**
     * @type {Map<String, Note[]>} A map txid->Notes, storing incoming spendable notes.
     * @private
     */
    private pendingUnspentNotes;
    save(): Promise<string>;
    /**
     * Load shieldWorker from a shieldData
     * @param {ShieldData} shieldData - shield data
     */
    load(shieldData: ShieldData): Promise<boolean>;
    /**
     * Loop through the txs of a block and update useful shield data
     * @param {{txs: String[], height: Number}} blockJson - Json of the block outputted from any PIVX node
     */
    handleBlock(blockJson: {
        txs: string[];
        height: number;
    }): Promise<void>;
    /**
     * Adds a transaction to the tree. Decrypts notes and stores nullifiers
     * @param {String} hex - transaction hex
     */
    addTransaction(hex: string, decryptOnly?: boolean): Promise<any>;
    /**
     * Remove the Shield Notes that match the nullifiers given in input
     * @param {Array<String>} blockJson - Array of nullifiers
     */
    removeSpentNotes(nullifiers: any): Promise<void>;
    /**
     * Return number of shield satoshis of the account
     */
    getBalance(): any;
    /**
     * Return number of pending satoshis of the account
     */
    getPendingBalance(): any;
    /**
     * Creates a transaction, sending `amount` satoshis to the address
     * @param {{address: String, amount: Number, blockHeight: Number, useShieldInputs: bool, utxos: UTXO[]?, transparentChangeAddress: String?}} target
     * @returns {{hex: String, spentUTXOs: UTXO[]}}
     */
    createTransaction({ address, amount, blockHeight, useShieldInputs, utxos, transparentChangeAddress, }: {
        address: string;
        amount: number;
        blockHeight: number;
        useShieldInputs: bool;
        utxos: UTXO[] | null;
        transparentChangeAddress: string | null;
    }): {
        hex: string;
        spentUTXOs: UTXO[];
    };
    getTxStatus(): Promise<any>;
    /**
     * Signals the class that a transaction was sent successfully
     * and the notes can be marked as spent
     * @throws Error if txid is not found
     * @param{String} txid - Transaction id
     */
    finalizeTransaction(txid: string): Promise<void>;
    /**
     * Discards the transaction, for example if
     * there were errors in sending them.
     * The notes won't be marked as spent.
     * @param{String} txid - Transaction id
     */
    discardTransaction(txid: string): void;
    /**
     * @returns {String} new shield address
     */
    getNewAddress(): string;
    loadSaplingProver(): Promise<any>;
    /**
     * @returns {Number} The last block that has been decoded
     */
    getLastSyncedBlock(): number;
}
export class Note {
    /**
     * Class corresponding to an unspent sapling shield note
     * @param {Array<Number>} o.recipient - Recipient PaymentAddress encoded as a byte array
     * @param {Number} o.value - How much PIVs are in the note
     * @param {Array<Number>} o.rseed - Random seed encoded as a byte array
     */
    constructor({ recipient, value, rseed }: Array<number>);
    recipient: any;
    value: any;
    rseed: any;
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
    constructor({ txid, vout, amount, privateKey, script }: {
        txid: string;
        vout: number;
        amount: number | null;
        privateKey: string | null;
        script: Uint8Array | null;
    });
    txid: string;
    vout: number;
    amount: number;
    private_key: any;
    script: Uint8Array;
}
declare class ShieldData {
    /**
     * Add a transparent UTXO, along with its private key
     * @param {Object} o - Options
     * @param {String} o.defaultAddress - Default shield address used for double check that data matches the seed
     * @param {Number} o.lastProcessedBlock - Last processed block in blockchain
     * @param {String} o.commitmentTree - Hex encoded commitment tree
     * @param {Uint8Array} o.diversifierIndex - Diversifier index of the last generated address
     * @param {[Note, String][]} o.unspentNotes - Array of notes, corresponding witness
     */
    constructor({ defaultAddress, lastProcessedBlock, commitmentTree, diversifierIndex, unspentNotes, }: {
        defaultAddress: string;
        lastProcessedBlock: number;
        commitmentTree: string;
        diversifierIndex: Uint8Array;
        unspentNotes: [Note, string][];
    });
    defaultAddress: string;
    diversifierIndex: Uint8Array;
    lastProcessedBlock: number;
    commitmentTree: string;
    unspentNotes: [Note, string][];
}
export {};
