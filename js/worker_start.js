import init, * as shieldMan from "pivx-shielding";

const start = async () => {
  await init();
  await shieldMan.initThreadPool(navigator.hardwareConcurrency);
  self.postMessage("done");
};
start();

self.onmessage = async (msg) => {
  console.log("Doing work!");
  const { uuid, name, args } = msg.data;

  try {
    const res = await shieldMan[name](...args);
    self.postMessage({ uuid, res });
  } catch (e) {
    self.postMessage({ uuid, res: false });
    console.log("Work failed :(");
    console.error(e);
  }
};
