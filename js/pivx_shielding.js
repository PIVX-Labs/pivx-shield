export default class {
    static async create(blockHeight) {
	return new PIVXShielding(await import('pivx-shielding'));
    }

    /**
     * Creates
     */
    static async createFromSeed(seed, blockHeight) {
	throw new Error("Not implemented");
    }
    
    constructor(wasm) {
	this.wasm = wasm;
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
	throw new Error("Not implemented");
    }
}
