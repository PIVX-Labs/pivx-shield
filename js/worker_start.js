import init, * as shieldMan from 'pivx-shielding';

const start = async () => {
    await init();
    await shieldMan.initThreadPool(navigator.hardwareConcurrency);
    self.postMessage('done');
}
start();

self.onmessage = async (msg) => {
    console.log("Doing work!");
    console.log(msg.data);
    try {
	const res = await shieldMan.create_transaction(msg.data);
	console.log(res);
    } catch (e) {
	console.log("Work failed :(");
	console.error(e);
    }
}
