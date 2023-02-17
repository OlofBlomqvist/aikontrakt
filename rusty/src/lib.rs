#[cfg(test)] use blockfrost::{BlockFrostApi, BlockFrostSettings, QueryParameters};


#[cfg(test)]
fn build_api() -> blockfrost::Result<BlockFrostApi> {
    let api = BlockFrostApi::new(
        std::fs::read_to_string("../blockfrost_apikey").expect("blockfrost_apikey file should exist in root directory"),
        BlockFrostSettings {
            network_address: "https://cardano-preview.blockfrost.io/api/v0".into(),
            query_parameters: QueryParameters::default(),
            retry_settings: blockfrost::RetrySettings::default()
        }
    );
    Ok(api)
}

#[cfg(test)]
#[tokio::test]
async fn unlock() {

    use cardano_serialization_lib::{
        tx_builder::tx_inputs_builder::{TxInputsBuilder, PlutusWitness},
        plutus::{PlutusScript, Redeemer, RedeemerTag, PlutusData, ExUnits, ConstrPlutusData, PlutusList}, 
        TransactionInput, crypto::{TransactionHash, Vkeywitnesses}, utils::{Value, to_bignum}, 
        address::Address, fees::LinearFee, Transaction
    };

    let api = build_api().unwrap();
    let my_addr = std::fs::read_to_string("../key.addr").expect("key.addr file should exist in root dir");
    let transaction_hash =  std::fs::read_to_string("../lock_tx_id").expect("lock_tx_id file should exist in root dir");
    let transactions_utxos = api.transactions_utxos(&transaction_hash).await.unwrap();
    
    let the_locked_utxo = 
        transactions_utxos.outputs.iter().find(|x|
            x.data_hash.is_some()
        ).unwrap();

    let some_other_output_for_collateral = 
        transactions_utxos.outputs.iter().find(|x|
            x.data_hash.is_none()
        ).unwrap();

    // Create the redeemer
    let mut lst = PlutusList::new();
    lst.add(&PlutusData::new_bytes(hex::encode("Hello, World!").into_bytes()));
    let redeemer = PlutusData::new_constr_plutus_data(
        & ConstrPlutusData::new(&to_bignum(0), &lst)
    );

    // The skey we got from generate_wallet.ts script
    let sk = std::fs::read_to_string("../key.sk").expect("lock.sk file should exist in root dir");
    let prv_key = cardano_serialization_lib::crypto::PrivateKey::from_bech32(&sk).unwrap();
    
    // Basic transaction builder
    let mut tx_builder = cardano_serialization_lib::tx_builder::TransactionBuilder::new(
        &cardano_serialization_lib::tx_builder::TransactionBuilderConfigBuilder::new()
            .fee_algo(&LinearFee::new(&to_bignum(44),&to_bignum(155381)))
            .pool_deposit(&to_bignum(500000000))
            .key_deposit(&to_bignum(2000000))
            .max_value_size(5000)
            .max_tx_size(16384)
            .coins_per_utxo_byte(&to_bignum(4310))
            .ex_unit_prices(&cardano_serialization_lib::plutus::ExUnitPrices::new(
                &cardano_serialization_lib::UnitInterval::new(&to_bignum(577), &to_bignum(10000)),
                &cardano_serialization_lib::UnitInterval::new(&to_bignum(721), &to_bignum(10000000)),
            ))
            .build().unwrap()
    );

    // Spend the utxo which is currently locked at the script address
    let mut input_builder = TxInputsBuilder::new();
    input_builder.add_plutus_script_input(
        &PlutusWitness::new_without_datum(
            &PlutusScript::from_bytes(
                hex::decode(
                    std::fs::read_to_string("../script_hex").expect("script_hex file should exist in root dir")
                ).unwrap()
            ).unwrap(), 
            &Redeemer::new(
                &RedeemerTag::new_spend(), 
                &to_bignum(0),
                &redeemer,
                &ExUnits::new(
                    &to_bignum(10000000),
                    &to_bignum(400000000)
                )
            ),
        ), 
        &TransactionInput::new(&TransactionHash::from_hex(&transaction_hash).unwrap(), 0), 
        &Value::new(
            &cardano_serialization_lib::utils::Coin::from_str(
                &the_locked_utxo.amount.first().unwrap().quantity
            ).unwrap()
        )
    );

    tx_builder.set_inputs(&input_builder);
    
    // We need to include some collateral when running plutus scripts
    let mut colatbuilder = TxInputsBuilder::new();
    colatbuilder.add_input(
        &Address::from_bech32(&my_addr).unwrap(), 
        &TransactionInput::new(&TransactionHash::from_hex(&transaction_hash).unwrap(), 1),
        &Value::new(
            &cardano_serialization_lib::utils::Coin::from_str(
                &some_other_output_for_collateral.amount.first().unwrap().quantity
            ).unwrap()
        )
    );

    tx_builder.set_collateral(&colatbuilder);
    
    //Dont really need this, we will just get everything back in a change utxo anyway
    // tx_builder.add_output(
    //     &cardano_serialization_lib::TransactionOutput::new(
    //         &Address::from_bech32(&my_addr).unwrap(),
    //     &Value::new(
    //         &cardano_serialization_lib::utils::Coin::from_str(
    //             "850000"    
    //         ).unwrap()
    //     )
    // )).unwrap();
    
    // The contract requires us to include this
    tx_builder.add_required_signer(&prv_key.to_public().hash());


    // Calc that hash why not
    tx_builder.calc_script_data_hash(
        &cardano_serialization_lib::tx_builder_constants::TxBuilderConstants::plutus_vasil_cost_models()
    ).unwrap();
    
    // Anything left without an output in this tx... we want it!
    tx_builder.add_change_if_needed(&Address::from_bech32(&my_addr).unwrap()).unwrap();
    
    // Finally we are getting somewhere..
    let tx = tx_builder.build_tx().unwrap();

    // Sign it using our private key
    let tx_hash = cardano_serialization_lib::utils::hash_transaction(&tx.body());
    let sig = cardano_serialization_lib::utils::make_vkey_witness(&tx_hash, &prv_key);

    // Add sig to witness set...
    let mut wits = tx.witness_set();
    let mut sigs = Vkeywitnesses::new();
    sigs.add(&sig);
    wits.set_vkeys(&sigs);

    // There must be a better way?
    let signed_tx = Transaction::new(
        &tx.body(),
        &wits,
        None
    );

    // Anyway, off we go!
    println!("{}",signed_tx.to_json().unwrap());
    let result = api.transactions_submit(signed_tx.to_bytes()).await.unwrap();
    println!("{}",result);
    
}


