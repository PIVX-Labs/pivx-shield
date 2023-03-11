export default class PIVXShielding {
  /**
   * Creates a PIVXShielding object
   * @param {Array<Number>} seed - array of 32 bytes that represents a random seed
   * @param {Number} blockHeight - number representing the block height of creation of the wallet
   * @param {Number} coinType - number representing the coin type, 1 represents testnet
   * @param {Number} accountIndex - index of the account that you want to generate, by default is set to 0
   */
  static async createFromSeed(seed, blockHeight, coinType, accountIndex = 0) {
    const shieldMan = await import("pivx-shielding");
    const serData = {
      seed: seed,
      coin_type: coinType,
      account_index: accountIndex,
    };
    const extsk = shieldMan.generate_extended_spending_key_from_seed(serData);
    const isTestNet = coinType == 1 ? true : false;
    const checkpointResult = shieldMan.get_closest_checkpoint(
      blockHeight,
      isTestNet
    );
    const effectiveHeight = checkpointResult[0];
    const commitmentTree = checkpointResult[1];
    return new PIVXShielding(
      shieldMan,
      extsk,
      isTestNet,
      effectiveHeight,
      commitmentTree
    );
  }

  constructor(shieldMan, extsk, isTestNet, blockHeight, commitmentTree) {
    this.shieldMan = shieldMan;
    this.extsk = extsk;
    this.generatedAddresses = 0;
    this.isTestNet = isTestNet;
    this.lastBlock = blockHeight;
    this.commitmentTree = commitmentTree;
    this.unspentNotes = [];
    this.nullifiers = []; // TODO: I have in mind a better method that does not need to save this
  }

  /**
   * Loop through the txs of a block and update useful shield data
   * @param {JSON} blockJson - Json of the block outputted from any PIVX node
   */
  handleBlock(blockJson) {
    for (let tx of blockJson.tx) {
      this.addTransaction(tx.hex);
    }
  }
  /**
   * Adds a transaction to the tree. Decrypts notes and stores nullifiers
   * @param {String} hex - transaction hex
   */
  addTransaction(hex) {
    //ONLY FOR TESTING RIGHT VALUES ARE A COMMENT FOR THE MOMENT
    let test_tree =
      "018c325f63f5cc98541cfef957f64845c86cf928e317ecc71a14debd364c7b8f57013c6f50deb5f788d5ac9105915ab9cbcda21a101d267c6424aa75b6e8df969e480d00016a2b0e3728a820b7982d81c87b80468ce65a4081843b890307115ca896416f3901e105bf42db29eca36e7235bd55546726753d1f967c3f284e243cbb3b3375d95a01a3ce8339e68a22d91b0750ef45468efe763e3d5e3a6e59809ddadcc94fe73c6c01494803bd8e6b730cb277701c613a4e7355cb54f79653724618e1436c02fca30c0177d25b5ed812af45eb46b54bc37c3fbe08fdfb4d952bb917fe59187bc78c42640001088b8a9fc4769017f3fdf865637e5cebbeaf7a4c643247723bf009da5eb1e4340001e877753448933a336fcf9399cc3dcd357344510c79db717e976979cb2eab612d0001cb846820acd916b4ea03b0a222b3eae8704bbd5365f105156041c578bd214c3201e03719b3810c7a9eaf6680ad3c60fb5ffdb0106975c952ef173c3e8cde943b03";
    let test_tx =
      "0300000001a7d31ea039ab2a9914be2a84b6e8966758da5f8d1a64ac6fb49d2763dccc38da000000006b483045022100bb67345313edea3c7462c463ea8e03ef3b14caccfbefff9877ef246138427b6a02200b74211e1f27be080561c3985980e6b0e2e833f0751ea68dfb1e465b994afefc0121025c6802ec58464d8e65d5f01be0b7ce6e8404e4a99f28ea3bfe47efe40df9108cffffffff01e8657096050000001976a914b4f73d5c66d999699a4c38ba9fe851d7141f1afa88ac0000000001003665c4ffffffff00010b39ab5d98de6f5e3f50f3f075f61fea263b4cdd6927a53ac92c196be72911237f5041af34fed06560b8620e16652edf6d297d14a9cff2145731de6643d4bf13e189dbc4b6c4b91fe421133a2f257e5b516efd9b080814251ec0169fabdac1ce4a14575d3a42a7ca852c1ef6f6e1f3daf60e9ae4b77ef4d9a589dcbc09e8437fc28e80d6a0c4f1627b3e34ee9dd5cd587d1d57bab30e0a2eba893a6b61d7e53f5b49b4cb67a807e5db203b76744025d8395c83be2eb71009f9b82e78e7b65d9740340106ee59b22cd3628f1f10c3712c2b4f86464b627b27910cd3e0a80c5387798db4f15f751b5886beb1ab1a8c298185ed6f5d3a074689ba6e327e8dc2bd3b41790ecbe0240f909b8735b8ac98a59855b448e9f37d31d5d25b71959264c145abd15f0606ab5844391819afd4017890696272abad451dab8654d76e41c389941f0fd134d7d6e3b971b15cc63ba9bea421383639bdbeaa970636d637a1c6167154f39ded089d0f07776c58e8e86c0dac8259d22644e9d8a89456e9ccf2f66ce8633a9055f1703669c6a7b009865347ef608cb4ba8f3158e05947580ec50c32f69c0079dff58b3b53367f43490d4bcaba946ef4c42b4d366c66184f84ec442499a056b6b60eeaee94151459ac0b61eb6debfa96554bbe8ec39d2c49ee6eca48ed8dc137f84584803e2372ec35e0f9f4252beef9170419e703183fa87e7d35c2403b41700bc9f5d69da6c01c870515694f5c48372cba6bacd6a79ca1cdb85f38841f7680d0dd6853b22fc95d6e307419271edb05f2f40733c31c6f827eca592658716c5c73a9dd00a7e387250beffaa78bd1f104e031e00f014f9a50935864e11ffd655ea4d4c6c3d80b681e7581a19b2668c00528110ee5322add9dacb35b519280812050061788884cad7cc409a9261e86485cc4f2d904bdf40b3c78208a395a2488eb938b8a198b51ac418fa79e5d1d7bd8f96fe0910fe61136d8fe302f144745a988d6de83e89cd8befef8a762103aa32a14d93e3ac41b44188ab385b65c1f21cf29f19a6d2af556385dd60a994ecd1ac909488f7abce29e26690651a389d4466a9e20b7f08bfbdf4f4aa3e1577dc7debf1951688db8c75347d01e836f7816df3c7a7aaa833cbd6309d179d5dfc34045e52984cf475890f04b2ffcdf123175cc07568d08d9b8525ad9eabad231e1549a19fdce0fbb30c1fe7ec59bf8ed8e642ec6571456bdba8ade4458cffb1d65fee35242d7409de14a21514416a68e9c2d5c21eb9ca5813e1d8162a48d650ed7696b7b14b4f3d3f5eb892cf32614f62dea794e7f68e6d3d3ae6edf22e811f85e1ac7fe2a8437bdf287aa4d5ff842173039074516304042370a4e2feeb901319665ffc9b005b37c2afbea22faca316ea4f6b5f365fe46f679581966dadd029d687d2b400201";
    let test_addr =
      "p-secret-spending-key-test1qd7a5dwjqqqqpqyzy3xs3usw7rzal27gvx6szvt56qff69ceqxtzdst9cuvut3n7dcp28wk2why35qd3989hdvf5wq9m62q6xfrmnlkf0r70v2s7x63sr2zzt8shr6psry8sq66kvzwskrghutgd7wmqknsljq0j0t2kmyg8xzqweug0pg40ml0s8mvvmgmp9c9pdvmpnx9gnhwde9yrg4f2c36c808d6p29ywevmn47lp9elyawr93wxl96ttd5pevj6f68qc6rcps5u9990";
    let res = this.shieldMan.handle_transaction(
      test_tree, //this.commitmentTree
      test_tx, //hex
      test_addr, //this.extsk,
      this.isTestNet
    );
    this.commitmentTree = res.commitment_tree;
    for (let x of res.decrypted_notes) {
      this.unspentNotes.push(x);
    }
    for (let x of res.nullifiers) {
      this.nullifiers.push(x);
    }
  }

  /**
   * Remove the Shield Notes that match the nullifiers given in input
   * @param {Array<String>} blockJson - Array of nullifiers 
   */
  removeSpentNotes(nullifiers){
    throw new Error("Not implemented");
  }
  /**
   * Return number of shielded satoshis of the account
   */
  getBalance() {
    let tot = 0;
    for (let x of this.unspentNotes) {
      tot += x[0].value;
    }
    return tot;
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
    const address = this.shieldMan.generate_next_shielding_payment_address(
      this.extsk,
      this.generatedAddresses + 1,
      this.isTestNet
    );
    this.generatedAddresses += 1;
    return address;
  }
}
