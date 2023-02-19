Just a toy repo for playing around with Aiken contracts using Lucid & Rust/CSL/CML.

Prereqs: 
1. Install Deno & optionally Rust/Cargo.
2. Add file "blockfrost_apikey" containing your key (for preview network)!

How to:

1. Initialize a wallet to use with the contract
    ```
    deno run --allow-net --allow-write --allow-read init.ts
    ```
    *(After running init.ts, you need to go to the faucet to fund the address)*

2. Lock an utxo in the script
    ```
    deno run --allow-net --allow-write --allow-read lock.ts
    ```

3. Unlock an utxo from the script

    Using Lucid:
    ```
    deno run --allow-net --allow-write --allow-read unlock.ts
    ```

    Using Rust:
    ```
    cargo test unlock --manifest-path=rusty/Cargo.toml -- --nocapture
    ``` 



