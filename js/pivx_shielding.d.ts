export class PIVXShielding {
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
    static create({ seed, extendedSpendingKey, blockHeight, coinType, accountIndex, loadSaplingData, }: {
        seed: Array<number> | null;
        extendedSpendingKey: string | null;
        blockHeight: number;
        coinType: number;
        accountIndex: number;
        loadSaplingData: boolean;
    }): Promise<PIVXShielding>;
    constructor(shieldMan: any, extsk: any, isTestNet: any, nHeight: any, commitmentTree: any);
    /**
     * Webassembly object that holds Shield related functions
     * @private
     */
    private shieldMan;
    /**
     * Extended spending key
     * @type {String}
     * @private
     */
    private extsk;
    /**
     * Number of generated addresses
     * @type {Number}
     * @private
     */
    private generatedAddresses;
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
     * Loop through the txs of a block and update useful shield data
     * @param {{txs: String[], height: Number}} blockJson - Json of the block outputted from any PIVX node
     */
    handleBlock(blockJson: {
        txs: string[];
        height: number;
    }): void;
    /**
     * Adds a transaction to the tree. Decrypts notes and stores nullifiers
     * @param {String} hex - transaction hex
     */
    addTransaction(hex: string): void;
    /**
     * Remove the Shield Notes that match the nullifiers given in input
     * @param {Array<String>} blockJson - Array of nullifiers
     */
    removeSpentNotes(nullifiers: any): void;
    /**
     * Return number of shielded satoshis of the account
     */
    getBalance(): any;
    /**
     * Creates a transaction, sending `amount` satoshis to the address
     * @param {{address: String, amount: Number, blockHeight: Number, useShieldInputs: bool, utxos: UTXO[]?}} target
     * @returns {{hex: String, spentUTXOs: UTXO[]}}
     */
    createTransaction({ address, amount, blockHeight, useShieldInputs, utxos, }: {
        address: string;
        amount: number;
        blockHeight: number;
        useShieldInputs: bool;
        utxos: UTXO[] | null;
    }): {
        hex: string;
        spentUTXOs: UTXO[];
    };
    /**
     * Signals the class that a transaction was sent successfully
     * and the notes can be marked as spent
     * @throws Error if txid is not found
     * @param{String} txid - Transaction id
     */
    finalizeTransaction(txid: string): void;
    /**
     * Discards the transaction, for example if
     * there were errors in sending them.
     * The notes won't be marked as spent.
     * @param{String} txid - Transaction id
     */
    discardTransaction(txid: string): void;
    /**
     * @returns {String} new shielded address
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
