import {
    Blockfrost,
    C,
    Constr,
    Data,
    Lucid,
    SpendingValidator,
    TxHash,
    fromHex,
    toHex,
    utf8ToHex,
  } from "https://deno.land/x/lucid@0.8.3/mod.ts";
  import * as cbor from "https://deno.land/x/cbor@v1.4.1/index.js";
   
  const lucid = await Lucid.new(
    new Blockfrost(
      "https://cardano-preview.blockfrost.io/api/v0",
      await Deno.readTextFile("./blockfrost_apikey"),
    ),
    "Preview",
  );
   
  lucid.selectWalletFromPrivateKey(await Deno.readTextFile("./key.sk"));
   
  const validator = await readValidator();
  Deno.writeTextFile("script_hex", validator.script);
  // --- Supporting functions
   
  async function readValidator(): Promise<SpendingValidator> {
    const validator = JSON.parse(await Deno.readTextFile("hello_world/plutus.json")).validators[0];
    return {
      type: "PlutusV2",
      script: toHex(cbor.encode(fromHex(validator.compiledCode))),
    };
  }


  // ^^^ Code above is unchanged. ^^^
 
const publicKeyHash = lucid.utils.getAddressDetails(
    await lucid.wallet.address()
  ).paymentCredential.hash;
   
  const datum = Data.to(new Constr(0, [publicKeyHash]));
   
  const txLock = await lock(1200000, { into: validator, owner: datum });
   
  await lucid.awaitTx(txLock);
   
  console.log(`1 ADA locked into the contract
      Tx ID: ${txLock}
      Datum: ${datum}
  `);

  Deno.writeTextFile("lock_tx_id", txLock);


   
  // --- Supporting functions
   
  async function lock(lovelace, { into, owner }): Promise<TxHash> {
    const contractAddress = lucid.utils.validatorToAddress(into);
   
    const tx = await lucid
      .newTx()
      .payToContract(contractAddress, { inline: owner }, { lovelace })
      .complete();
   
    const signedTx = await tx.sign().complete();
    //console.log("submitting tx...",tx.txComplete.to_json())
    return signedTx.submit();
  }