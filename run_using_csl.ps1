<#
    This script will initialize a new wallet when you first run it.
    (Remove the key* files if you wish to re-generate)

    It will then generate and submit a transaction that locks a utxo using the
    aiken hello-world script. After that we also generate and submit a transaction
    that unlocks the utxo.
#>


if($null -eq (get-command deno)) {
    throw "Please install deno before running this script"
}

if((test-path key.*) -eq $false) {
    Write-Output "Generating a wallet just for you!"
    deno run --allow-net --allow-write --allow-read init.ts
    if($LASTEXITCODE -ne 0) {throw "failed to initialize wallet"}    
    Write-Output "Go over here and request funds to your new address: https://docs.cardano.org/cardano-testnet/tools/faucet"    
}

$skey = Get-Content -Raw key.sk
$addr = Get-Content -Raw key.addr

Write-Host "Using address $addr"
Write-Host "Your private key: $skey"

if(test-path "lock_tx_id") {
    Remove-Item "lock_tx_id" -ErrorAction stop
}

# # TODO : Lock also using csl!
Write-Host "Locking some funds!"
deno run --allow-net --allow-read --allow-write lock.ts
if($LASTEXITCODE -ne 0) {throw "failed to lock funds!"}    

Write-Host "Sleeping for some 15 seconds to give the utxo some time to materialize.."
Start-Sleep 15

Write-Host "Unlocking the funds!"
cargo test --manifest-path=rusty/Cargo.toml





