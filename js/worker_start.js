import { threads } from "wasm-feature-detect";
let shieldMan = null;

const start = async () => {
  if (await threads()) {
    shieldMan = await import("pivx-shielding-multicore");
    await shieldMan.default();
    await shieldMan.initThreadPool(navigator.hardwareConcurrency);
  } else {
    shieldMan = await import("pivx-shielding");
    await shieldMan.default();
  }
  self.postMessage("done");
};

start();

self.onmessage = async (msg) => {
  const { uuid, name, args } = msg.data;

  try {
    const res = await shieldMan[name](...args);
    self.postMessage({ uuid, res });
  } catch (e) {
    self.postMessage({ uuid, rej: e });
    throw e;
  }
};
