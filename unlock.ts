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
    utf8ToHex
  } from "https://deno.land/x/lucid@0.8.3/mod.ts";
import * as cbor from "https://deno.land/x/cbor@v1.4.1/index.js";

const provider =  new Blockfrost(
  "https://cardano-preview.blockfrost.io/api/v0",
  await Deno.readTextFile("./blockfrost_apikey"),
);

const lucid = await Lucid.new(
 provider,
  "Preview",
);
  
lucid.selectWalletFromPrivateKey(await Deno.readTextFile("./key.sk"));
  
const validator = await readValidator();
  
  
async function readValidator(): Promise<SpendingValidator> {
  const validator = JSON.parse(await Deno.readTextFile("hello_world/plutus.json")).validators[0];
  return {
    type: "PlutusV2",
    script: toHex(cbor.encode(fromHex(validator.compiledCode))),
  };
}

const txHash = await Deno.readTextFile("lock_tx_id")

const utxo = { txHash: txHash, outputIndex: 1 };
 
const redeemer = Data.to(new Constr(0, [utf8ToHex("Hello, World!")]));
console.log(redeemer)

const txUnlock = await unlock(utxo, { from: validator, using: redeemer });
 
await lucid.awaitTx(txUnlock);
 
console.log(`1 ADA recovered from the contract
    Tx ID: ${txUnlock}
    Redeemer: ${redeemer}
`);

async function unlock(ref, { from, using }): Promise<TxHash> {
  const [utxo] = await lucid.utxosByOutRef([ref]);
 
  const tx = await lucid
    .newTx()
    .collectFrom([utxo], using)
    .addSigner(await lucid.wallet.address())
    .attachSpendingValidator(from)
    .complete();
 
  const signedTx = await tx
    .sign()
    .complete();

  //console.log("submitting tx",tx.txComplete.to_json())
  Deno.writeTextFile("unlock_tx.json", signedTx.txSigned.to_json());
  
  return signedTx.submit();
}