import init, * as shieldMan from "pivx-shielding";

const start = async () => {
  await init();
  if (shieldMan.initThreadPool)  
    await shieldMan.initThreadPool(navigator.hardwareConcurrency);
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
