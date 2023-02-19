#[cfg(test)] use blockfrost::{BlockFrostApi, BlockFrostSettings, QueryParameters};
use cardano_serialization_lib::{plutus::{Costmdls, Language, CostModel}, tx_builder::{TransactionBuilder}, Transaction, crypto::{Vkeywitnesses}};


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
        TransactionInput, crypto::{TransactionHash}, utils::{Value, to_bignum, BigNum}, 
        address::Address, fees::{LinearFee}
    };

    let api = build_api().unwrap();
    let my_addr = std::fs::read_to_string("../key.addr").expect("key.addr file should exist in root dir");
    let transaction_hash =  std::fs::read_to_string("../lock_tx_id").expect("lock_tx_id file should exist in root dir");
    let transactions_utxos = api.transactions_utxos(&transaction_hash).await.unwrap();
    let my_addr_obj = Address::from_bech32(&my_addr).unwrap();

    let some_other_output_for_collateral = transactions_utxos.outputs.get(2).unwrap();
    let the_locked_utxo = transactions_utxos.outputs.get(1).unwrap();    

    let mut lst = PlutusList::new();
    let rawbytes = "Hello, World!".as_bytes().to_vec();
    let redeemer_bytes = PlutusData::new_bytes(rawbytes.clone());
    lst.add(&redeemer_bytes);
    let redeemer = PlutusData::new_constr_plutus_data(
        &ConstrPlutusData::new(&to_bignum(0), &lst)
    );

    let sk = std::fs::read_to_string("../key.sk").expect("lock.sk file should exist in root dir");
    let prv_key = cardano_serialization_lib::crypto::PrivateKey::from_bech32(&sk).unwrap();

    let themparams = api.epochs_latest_parameters().await.unwrap();
    
    let linfee = 
        LinearFee::new(&to_bignum(themparams.min_fee_a.try_into().unwrap()),&to_bignum(themparams.min_fee_b.try_into().unwrap()));
    let mut tx_builder = cardano_serialization_lib::tx_builder::TransactionBuilder::new(
        &cardano_serialization_lib::tx_builder::TransactionBuilderConfigBuilder::new()
            .fee_algo(&linfee)
            .pool_deposit(&BigNum::from_str(&themparams.pool_deposit).unwrap())
            .key_deposit(&BigNum::from_str(&themparams.key_deposit).unwrap())
            .max_value_size(themparams.max_val_size.parse::<u32>().unwrap())
            .max_tx_size(themparams.max_tx_size as u32)
            .coins_per_utxo_byte(&BigNum::from_str(&themparams.coins_per_utxo_size).unwrap())
            .ex_unit_prices(&cardano_serialization_lib::plutus::ExUnitPrices::new(
                &cardano_serialization_lib::UnitInterval::new(&to_bignum(themparams.price_mem as u64), &to_bignum(10000)),
                &cardano_serialization_lib::UnitInterval::new(&to_bignum(themparams.price_step as u64), &to_bignum(10000000)),
            ))
            .build().unwrap()
    );

    let mut input_builder = TxInputsBuilder::new();
    let actual_redeemer = Redeemer::new(
        &RedeemerTag::new_spend(), 
        &to_bignum(0),
        &redeemer,
        &ExUnits::new(
            &to_bignum(51717),
            &to_bignum(19823760)
        )
    );


    input_builder.add_plutus_script_input(
        &PlutusWitness::new_without_datum(
            &PlutusScript::from_bytes_v2(
                hex::decode(
                    std::fs::read_to_string("../script_hex").expect("script_hex file should exist in root dir")
                ).unwrap()
            ).unwrap(), 
            &actual_redeemer,
        ), 
        &TransactionInput::new(
            &TransactionHash::from_hex(&transaction_hash).unwrap(), 
            the_locked_utxo.output_index as u32
        ), 
        &Value::new(
            &cardano_serialization_lib::utils::Coin::from_str(
                &the_locked_utxo.amount.first().unwrap().quantity
            ).unwrap()
        )
    );
    
    let total_inputs = 
        the_locked_utxo.amount.first().unwrap().quantity.parse::<u64>().unwrap() + 
        some_other_output_for_collateral.amount.first().unwrap().quantity.parse::<u64>().unwrap();

    let explicit_min_fee_since_we_are_too_lazy_to_fix_the_fee_calc : u64 = 200_000;
    
    let tot_minus_fee = cardano_serialization_lib::utils::Coin::from(
        total_inputs-explicit_min_fee_since_we_are_too_lazy_to_fix_the_fee_calc
    );

    tx_builder.add_output(
        &cardano_serialization_lib::TransactionOutput::new(&my_addr_obj,&Value::new(&tot_minus_fee))
    ).unwrap();
    
    tx_builder.add_required_signer(&prv_key.to_public().hash());

    let mut colatbuilder = TxInputsBuilder::new();
    let colin = TransactionInput::new(&
        TransactionHash::from_hex(&transaction_hash).unwrap(), 
        some_other_output_for_collateral.output_index as u32);
    let coval = Value::new(
        &cardano_serialization_lib::utils::Coin::from_str(
            &some_other_output_for_collateral.amount.first().unwrap().quantity
        ).unwrap()
    );

    let costmod = make_cost_model(themparams.cost_models.clone());

    colatbuilder.add_input(&my_addr_obj,&colin,&coval);   
    input_builder.add_input(&my_addr_obj,&colin,&coval);
    tx_builder.set_inputs(&input_builder);      
    tx_builder.set_collateral(&colatbuilder);  

    
    tx_builder.calc_script_data_hash(
        &costmod
    ).unwrap();

    tx_builder.set_total_collateral_and_return(
        &cardano_serialization_lib::utils::Coin::from_str(
            &some_other_output_for_collateral.amount.first().unwrap().quantity
        ).unwrap(), 
        &my_addr_obj
    ).unwrap();
    
    tx_builder.add_change_if_needed(&my_addr_obj).unwrap();

    let signed_tx = sign_tx(&tx_builder);
    
    // off we go!
    std::fs::write("../rust_unlock_tx.json", signed_tx.to_json().unwrap()).unwrap();
    println!("{}",signed_tx.to_json().unwrap());
    let result = api.transactions_submit(signed_tx.to_bytes()).await.unwrap();
    println!("{}",result);
    
}


pub fn sign_tx(tx_builder:&TransactionBuilder) -> Transaction {
    let sk = std::fs::read_to_string("../key.sk").expect("lock.sk file should exist in root dir");
    let prv_key = cardano_serialization_lib::crypto::PrivateKey::from_bech32(&sk).unwrap();
    let tx = tx_builder.build_tx().unwrap();
    let tx_hash = cardano_serialization_lib::utils::hash_transaction(&tx_builder.build().unwrap());
    let sig = cardano_serialization_lib::utils::make_vkey_witness(&tx_hash, &prv_key);
    let mut wits = tx.witness_set();
    let mut sigs = Vkeywitnesses::new();
    sigs.add(&sig);
    wits.set_vkeys(&sigs);
    Transaction::new(
        &tx_builder.build().unwrap(),
        &wits,
        None
    )
}





pub fn make_cost_model(x:blockfrost::CostModels) -> Costmdls {
    let mut res = Costmdls::new();
    res.insert(
        &Language::new_plutus_v1(),
        &CostModel::from(vec![
            205665, 812, 1, 1, 1000, 571, 0, 1, 1000, 24177, 4, 1, 1000, 32, 117366, 10475, 4,
            23000, 100, 23000, 100, 23000, 100, 23000, 100, 23000, 100, 23000, 100, 100, 100,
            23000, 100, 19537, 32, 175354, 32, 46417, 4, 221973, 511, 0, 1, 89141, 32, 497525,
            14068, 4, 2, 196500, 453240, 220, 0, 1, 1, 1000, 28662, 4, 2, 245000, 216773, 62,
            1, 1060367, 12586, 1, 208512, 421, 1, 187000, 1000, 52998, 1, 80436, 32, 43249, 32,
            1000, 32, 80556, 1, 57667, 4, 1000, 10, 197145, 156, 1, 197145, 156, 1, 204924,
            473, 1, 208896, 511, 1, 52467, 32, 64832, 32, 65493, 32, 22558, 32, 16563, 32,
            76511, 32, 196500, 453240, 220, 0, 1, 1, 69522, 11687, 0, 1, 60091, 32, 196500,
            453240, 220, 0, 1, 1, 196500, 453240, 220, 0, 1, 1, 806990, 30482, 4, 1927926,
            82523, 4, 265318, 0, 4, 0, 85931, 32, 205665, 812, 1, 1, 41182, 32, 212342, 32,
            31220, 32, 32696, 32, 43357, 32, 32247, 32, 38314, 32, 9462713, 1021, 10,
        ]),
    );
    res.insert(
        &Language::new_plutus_v2(),
        &CostModel::from(vec![
            x.plutus_v2.add_integer_cpu_arguments_intercept as i128,
            x.plutus_v2.add_integer_cpu_arguments_slope as i128,
            x.plutus_v2.add_integer_memory_arguments_intercept as i128,
            x.plutus_v2.add_integer_memory_arguments_slope as i128,
            x.plutus_v2.append_byte_string_cpu_arguments_intercept as i128,
            x.plutus_v2.append_byte_string_cpu_arguments_slope as i128,
            x.plutus_v2.append_byte_string_memory_arguments_intercept as i128,
            x.plutus_v2.append_byte_string_memory_arguments_slope as i128,
            x.plutus_v2.append_string_cpu_arguments_intercept as i128,
            x.plutus_v2.append_string_cpu_arguments_slope as i128,
            x.plutus_v2.append_string_memory_arguments_intercept as i128,
            x.plutus_v2.append_string_memory_arguments_slope as i128,
            x.plutus_v2.b_data_cpu_arguments as i128,
            x.plutus_v2.b_data_memory_arguments as i128,
            x.plutus_v2.blake2b_256_cpu_arguments_intercept as i128,
            x.plutus_v2.blake2b_256_cpu_arguments_slope as i128,
            x.plutus_v2.blake2b_256_memory_arguments as i128,
            x.plutus_v2.cek_apply_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_apply_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_builtin_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_builtin_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_const_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_const_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_delay_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_delay_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_force_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_force_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_lam_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_lam_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_startup_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_startup_cost_ex_budget_memory as i128,
            x.plutus_v2.cek_var_cost_ex_budget_cpu as i128,
            x.plutus_v2.cek_var_cost_ex_budget_memory as i128,
            x.plutus_v2.choose_data_cpu_arguments as i128,
            x.plutus_v2.choose_data_memory_arguments as i128,
            x.plutus_v2.choose_list_cpu_arguments as i128,
            x.plutus_v2.choose_list_memory_arguments as i128,
            x.plutus_v2.choose_unit_cpu_arguments as i128,
            x.plutus_v2.choose_unit_memory_arguments as i128,
            x.plutus_v2.cons_byte_string_cpu_arguments_intercept as i128,
            x.plutus_v2.cons_byte_string_cpu_arguments_slope as i128,
            x.plutus_v2.cons_byte_string_memory_arguments_intercept as i128,
            x.plutus_v2.cons_byte_string_memory_arguments_slope as i128,
            x.plutus_v2.constr_data_cpu_arguments as i128,
            x.plutus_v2.constr_data_memory_arguments as i128,
            x.plutus_v2.decode_utf8_cpu_arguments_intercept as i128,
            x.plutus_v2.decode_utf8_cpu_arguments_slope as i128,
            x.plutus_v2.decode_utf8_memory_arguments_intercept as i128,
            x.plutus_v2.decode_utf8_memory_arguments_slope as i128,
            x.plutus_v2.divide_integer_cpu_arguments_constant as i128,
            x.plutus_v2.divide_integer_cpu_arguments_model_arguments_intercept as i128,
            x.plutus_v2.divide_integer_cpu_arguments_model_arguments_slope as i128,
            x.plutus_v2.divide_integer_memory_arguments_intercept as i128,
            x.plutus_v2.divide_integer_memory_arguments_minimum as i128,
            x.plutus_v2.divide_integer_memory_arguments_slope as i128,
            x.plutus_v2.encode_utf8_cpu_arguments_intercept as i128,
            x.plutus_v2.encode_utf8_cpu_arguments_slope as i128,
            x.plutus_v2.encode_utf8_memory_arguments_intercept as i128,
            x.plutus_v2.encode_utf8_memory_arguments_slope as i128,
            x.plutus_v2.equals_byte_string_cpu_arguments_constant as i128,
            x.plutus_v2.equals_byte_string_cpu_arguments_intercept as i128,
            x.plutus_v2.equals_byte_string_cpu_arguments_slope as i128,
            x.plutus_v2.equals_byte_string_memory_arguments as i128,
            x.plutus_v2.equals_data_cpu_arguments_intercept as i128,
            x.plutus_v2.equals_data_cpu_arguments_slope as i128,
            x.plutus_v2.equals_data_memory_arguments as i128,
            x.plutus_v2.equals_integer_cpu_arguments_intercept as i128,
            x.plutus_v2.equals_integer_cpu_arguments_slope as i128,
            x.plutus_v2.equals_integer_memory_arguments as i128,
            x.plutus_v2.equals_string_cpu_arguments_constant as i128,
            x.plutus_v2.equals_string_cpu_arguments_intercept as i128,
            x.plutus_v2.equals_string_cpu_arguments_slope as i128,
            x.plutus_v2.equals_string_memory_arguments as i128,
            x.plutus_v2.fst_pair_cpu_arguments as i128,
            x.plutus_v2.fst_pair_memory_arguments as i128,
            x.plutus_v2.head_list_cpu_arguments as i128,
            x.plutus_v2.head_list_memory_arguments as i128,
            x.plutus_v2.i_data_cpu_arguments as i128,
            x.plutus_v2.i_data_memory_arguments as i128,
            x.plutus_v2.if_then_else_cpu_arguments as i128,
            x.plutus_v2.if_then_else_memory_arguments as i128,
            x.plutus_v2.index_byte_string_cpu_arguments as i128,
            x.plutus_v2.index_byte_string_memory_arguments as i128,
            x.plutus_v2.length_of_byte_string_cpu_arguments as i128,
            x.plutus_v2.length_of_byte_string_memory_arguments as i128,
            x.plutus_v2.less_than_byte_string_cpu_arguments_intercept as i128,
            x.plutus_v2.less_than_byte_string_cpu_arguments_slope as i128,
            x.plutus_v2.less_than_byte_string_memory_arguments as i128,
            x.plutus_v2.less_than_equals_byte_string_cpu_arguments_intercept as i128,
            x.plutus_v2.less_than_equals_byte_string_cpu_arguments_slope as i128,
            x.plutus_v2.less_than_equals_byte_string_memory_arguments as i128,
            x.plutus_v2.less_than_equals_integer_cpu_arguments_intercept as i128,
            x.plutus_v2.less_than_equals_integer_cpu_arguments_slope as i128,
            x.plutus_v2.less_than_equals_integer_memory_arguments as i128,
            x.plutus_v2.less_than_integer_cpu_arguments_intercept as i128,
            x.plutus_v2.less_than_integer_cpu_arguments_slope as i128,
            x.plutus_v2.less_than_integer_memory_arguments as i128,
            x.plutus_v2.list_data_cpu_arguments as i128,
            x.plutus_v2.list_data_memory_arguments as i128,
            x.plutus_v2.map_data_cpu_arguments as i128,
            x.plutus_v2.map_data_memory_arguments as i128,
            x.plutus_v2.mk_cons_cpu_arguments as i128,
            x.plutus_v2.mk_cons_memory_arguments as i128,
            x.plutus_v2.mk_nil_data_cpu_arguments as i128,
            x.plutus_v2.mk_nil_data_memory_arguments as i128,
            x.plutus_v2.mk_nil_pair_data_cpu_arguments as i128,
            x.plutus_v2.mk_nil_pair_data_memory_arguments as i128,
            x.plutus_v2.mk_pair_data_cpu_arguments as i128,
            x.plutus_v2.mk_pair_data_memory_arguments as i128,
            x.plutus_v2.mod_integer_cpu_arguments_constant as i128,
            x.plutus_v2.mod_integer_cpu_arguments_model_arguments_intercept as i128,
            x.plutus_v2.mod_integer_cpu_arguments_model_arguments_slope as i128,
            x.plutus_v2.mod_integer_memory_arguments_intercept as i128,
            x.plutus_v2.mod_integer_memory_arguments_minimum as i128,
            x.plutus_v2.mod_integer_memory_arguments_slope as i128,
            x.plutus_v2.multiply_integer_cpu_arguments_intercept as i128,
            x.plutus_v2.multiply_integer_cpu_arguments_slope as i128,
            x.plutus_v2.multiply_integer_memory_arguments_intercept as i128,
            x.plutus_v2.multiply_integer_memory_arguments_slope as i128,
            x.plutus_v2.null_list_cpu_arguments as i128,
            x.plutus_v2.null_list_memory_arguments as i128,
            x.plutus_v2.quotient_integer_cpu_arguments_constant as i128,
            x.plutus_v2.quotient_integer_cpu_arguments_model_arguments_intercept as i128,
            x.plutus_v2.quotient_integer_cpu_arguments_model_arguments_slope as i128,
            x.plutus_v2.quotient_integer_memory_arguments_intercept as i128,
            x.plutus_v2.quotient_integer_memory_arguments_minimum as i128,
            x.plutus_v2.quotient_integer_memory_arguments_slope as i128,
            x.plutus_v2.remainder_integer_cpu_arguments_constant as i128,
            x.plutus_v2.remainder_integer_cpu_arguments_model_arguments_intercept as i128,
            x.plutus_v2.remainder_integer_cpu_arguments_model_arguments_slope as i128,
            x.plutus_v2.remainder_integer_memory_arguments_intercept as i128,
            x.plutus_v2.remainder_integer_memory_arguments_minimum as i128,
            x.plutus_v2.remainder_integer_memory_arguments_slope as i128,
            x.plutus_v2.serialise_data_cpu_arguments_intercept as i128,
            x.plutus_v2.serialise_data_cpu_arguments_slope as i128,
            x.plutus_v2.serialise_data_memory_arguments_intercept as i128,
            x.plutus_v2.serialise_data_memory_arguments_slope as i128,
            x.plutus_v2.sha2_256_cpu_arguments_intercept as i128,
            x.plutus_v2.sha2_256_cpu_arguments_slope as i128,
            x.plutus_v2.sha2_256_memory_arguments as i128,
            x.plutus_v2.sha3_256_cpu_arguments_intercept as i128,
            x.plutus_v2.sha3_256_cpu_arguments_slope as i128,
            x.plutus_v2.sha3_256_memory_arguments as i128,
            x.plutus_v2.slice_byte_string_cpu_arguments_intercept as i128,
            x.plutus_v2.slice_byte_string_cpu_arguments_slope as i128,
            x.plutus_v2.slice_byte_string_memory_arguments_intercept as i128,
            x.plutus_v2.slice_byte_string_memory_arguments_slope as i128,
            x.plutus_v2.snd_pair_cpu_arguments as i128,
            x.plutus_v2.snd_pair_memory_arguments as i128,
            x.plutus_v2.subtract_integer_cpu_arguments_intercept as i128,
            x.plutus_v2.subtract_integer_cpu_arguments_slope as i128,
            x.plutus_v2.subtract_integer_memory_arguments_intercept as i128,
            x.plutus_v2.subtract_integer_memory_arguments_slope as i128,
            x.plutus_v2.tail_list_cpu_arguments as i128,
            x.plutus_v2.tail_list_memory_arguments as i128,
            x.plutus_v2.trace_cpu_arguments as i128,
            x.plutus_v2.trace_memory_arguments as i128,
            x.plutus_v2.un_bdata_cpu_arguments as i128,
            x.plutus_v2.un_bdata_memory_arguments as i128,
            x.plutus_v2.un_constr_data_cpu_arguments as i128,
            x.plutus_v2.un_constr_data_memory_arguments as i128,
            x.plutus_v2.un_idata_cpu_arguments as i128,
            x.plutus_v2.un_idata_memory_arguments as i128,
            x.plutus_v2.un_list_data_cpu_arguments as i128,
            x.plutus_v2.un_list_data_memory_arguments as i128,
            x.plutus_v2.un_map_data_cpu_arguments as i128,
            x.plutus_v2.un_map_data_memory_arguments as i128,
            x.plutus_v2.verify_ecdsa_secp256k1_signature_cpu_arguments as i128,
            x.plutus_v2.verify_ecdsa_secp256k1_signature_memory_arguments as i128,
            x.plutus_v2.verify_ed25519_signature_cpu_arguments_intercept as i128,
            x.plutus_v2.verify_ed25519_signature_cpu_arguments_slope as i128,
            x.plutus_v2.verify_ed25519_signature_memory_arguments as i128,
            x.plutus_v2.verify_schnorr_secp256k1_signature_cpu_arguments_intercept as i128,
            x.plutus_v2.verify_schnorr_secp256k1_signature_cpu_arguments_slope as i128,
            x.plutus_v2.verify_schnorr_secp256k1_signature_memory_arguments as i128

        ]),
    );
    res
}
