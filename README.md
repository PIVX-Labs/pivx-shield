# PIVX Shield
WASM library for interoperation with the PIVX Shield sapling protocol.
It supports:
- Generating addresses with the zip32 protocol,
- Shield and transparent transactions,
- Multicore support with backwards compatibility for older browsers
- Transaction progress status
- The ability to save and load the sync status

## Getting started
### Install
To install the library, simply run `npm i pivx-shield`. Then import it in your project with
```js
import { PIVXShield } from "pivx-shield";
```

### Examples

To use the class, it must first be synced with:
```js
import { PIVXShield as Shield } from "pivx-shield";

// This is an array of 64 random bytes, usually derived from a Seed phrase
// For instance with https://github.com/bitcoinjs/bip39
const yourSeed = ...;
const shield = await Shield.create({
	seed: yourSeed,
	// This should be the block of birth of the wallet.
	// Put 0 if you don't know when it was created,
	// If you have to guess, pick a date when you definetely
	// didn't have the wallet
	blockHeight: 1164767,
	coinType: 1, // Testnet
	accoutnIndex: 0,
});
// getLastSyncedBlock will start from the first checkpoint it finds. 
for (let block = shield.getLastSyncedBlock();  block < current_block_height; block++)  {
	// You need to provide a function that fetches block information
	// For example with a simple GET request to a blockbook explorer
	// https://testnet.rockdev.org/api/v2/block/1164637
	const blockData = your_fetch_block_function(block);
	await shield.handleBlock(blockData);
}
console.log(shield.getBalance());
console.log(await shield.getNewAddress());
```

You can also save and load the public data, for a faster sync

```js
// This return a string with the public shield data
const data = await shield.save();
localStorage.setItem("shield", data);
```

```js
const data = localStorage.getItem("shield");
const seed = ...;
const shield = await Shield.create({
	data,
	seed,
	// testnet
	coinType: 1,
	accountIndex: 0,
});

// Will return the block from when save was called
console.log(shield.getLastSyncedBlock());
```

To create a transaction,

```js
import { PIVXShield as Shield } from "pivx-shield";

const shield = await Shield.create({...});
// Sync omitted

// The library provides a hex encoded signed transaction, ready to be broadcast to the network.
// For example, with a standard PIVX node, `sendrawtransaction` can be used
// For more info,
// https://github.com/PIVX-Project/PIVX/wiki/Raw-Transactions#user-content-createrawtransaction_txidtxidvoutn_addressamount
const { hex }  = await shield.createTransaction({
	// Transparent addresses are supported as well
	address: "ptestsapling1s23gkjxqnedkptdvp8qn3m57z0meq2530qxwe8w7x9sdz05xg5yu8wh7534memvjwqntw8mzr3w",
	// 50 tPIV
	amount: 50 * 10**8,
	useShieldInputs: true,
});

// Let's assume your send transaction method returns the txid if successful or null if not
const txid = your_send_transaction_method(hex);
if (txid) {
	shield.finalizeTransaction(txid);
} else {
	shield.discardTransaction(txid);
}
```

## Contribuiting

PRs are welcome!
Write tests and then make sure to run `cargo fmt` before submitting

### Compile
To compile, run `make`.
This will generate two versions: `pkg/` and `pkg_multicore/`. Then, run `npm i /path/to/this/project/js/`, to install the javascript wrapper in your project.

## License

See LICENSE for licensing information
