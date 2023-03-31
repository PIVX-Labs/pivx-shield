# PIVX Shield
WASM library for interoperation with the PIVX Shield protocol.

## Getting started
### Compile
To compile the non-multicore version,
```bash
wasm-pack build --target web
```
To compile with multicore support enabled, make sure the nightly compiler and `rust-src` are installed. Version 2023-03-28 is recommended.
```bash
./build_multicore.sh
```
This will generate the two versions in `pkg/` and `pkg_multicore/`. Then, run `npm i /path/to/this/project/js/`, to install the javascript wrapper in your project.

### Examples

```js
import { PIVXShield as Shield } from "pivx-shield-js";
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
	await shield.handleBlock(your_fetch_block_function(block));
}
console.log(shield.getBalance());
console.log(await shield.getNewAddress());

const { hex } = await shield.createTransaction({
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

## License

See LICENSE for licensing information
