use crate::{
    assets::evm_test_contract::{
        EVM_CONTRACT_BYTECODE1, EVM_CONTRACT_BYTECODE1_METHOD_SAY_HELLO_WORLD_STR_ID,
    },
    core::utils::TestingContext,
};
use fluentbase_codec::Encoder;
use fluentbase_core::{consts::ECL_CONTRACT_ADDRESS, helpers::calc_create_address, Account};
use fluentbase_runtime::{DefaultEmptyRuntimeDatabase, RuntimeContext};
use fluentbase_sdk::LowLevelSDK;
use fluentbase_sdk::{
    CoreInput, EvmCallMethodInput, EvmCreateMethodInput, EVM_CALL_METHOD_ID, EVM_CREATE_METHOD_ID,
};
use fluentbase_types::{
    address, wasm2rwasm, Address, Bytes, ExitCode, IJournaledTrie, B256, STATE_DEPLOY, STATE_MAIN,
    U256,
};
use hex_literal::hex;

#[test]
fn test_evm_create() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_account = Account {
        address: caller_address,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let expected_contract_address = calc_create_address(&caller_address, caller_account.nonce);
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_code = EVM_CONTRACT_BYTECODE1;

    let value = B256::left_padding_from(&hex!("1000"));
    let gas_limit: u64 = 10_000_000;
    let evm_create_method_input = EvmCreateMethodInput {
        init_code: contract_input_code.into(),
        value: value.into(),
        gas_limit,
        salt: None,
    };
    let evm_create_core_input = CoreInput::new(EVM_CREATE_METHOD_ID, evm_create_method_input);
    let evm_create_core_input_vec = evm_create_core_input.encode_to_vec(0);

    const IS_RUNTIME: bool = true;
    let evm_contract_wasm_binary =
        include_bytes!("../../../crates/contracts/assets/ecl_contract.wasm");
    let evm_contract_rwasm_binary = wasm2rwasm(evm_contract_wasm_binary.as_slice()).unwrap();
    let mut runtime_ctx =
        RuntimeContext::<DefaultEmptyRuntimeDatabase>::new(evm_contract_rwasm_binary)
            .with_jzkt(LowLevelSDK::with_default_jzkt())
            .with_state(STATE_MAIN);
    let mut test_ctx = TestingContext::<IS_RUNTIME>::new();
    test_ctx.try_add_account(&caller_account);
    test_ctx
        .contract_input_wrapper
        .set_journal_checkpoint(runtime_ctx.jzkt().checkpoint().to_u64())
        .set_contract_input(Bytes::copy_from_slice(&evm_create_core_input_vec))
        .set_block_chain_id(env_chain_id)
        .set_contract_caller(caller_address)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address);
    test_ctx.apply_ctx(&mut runtime_ctx);

    let output = test_ctx.run_rwasm_with_input(runtime_ctx, false, gas_limit);
    assert_eq!(ExitCode::Ok.into_i32(), output.exit_code);
    let contract_address_vec = output.output;
    let contract_address = Address::from_slice(&contract_address_vec);

    assert_eq!(expected_contract_address, contract_address);
}

#[test]
fn test_evm_call_after_create() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let expected_contract_address = calc_create_address(&caller_address, caller_nonce);
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_code = EVM_CONTRACT_BYTECODE1;
    let gas_limit: u64 = 10_000_000;
    const IS_RUNTIME: bool = true;
    let ecl_wasm = include_bytes!("../../../crates/contracts/assets/ecl_contract.wasm");
    let ecl_rwasm = wasm2rwasm(ecl_wasm.as_slice()).unwrap();
    let create_value = B256::left_padding_from(&hex!("1000"));
    let call_value = B256::left_padding_from(&hex!("00"));

    let (jzkt, deployed_contract_address) = {
        let evm_create_method_input = EvmCreateMethodInput {
            init_code: contract_input_code.into(),
            value: create_value.into(),
            gas_limit,
            salt: None,
        };
        let evm_create_core_input = CoreInput::new(EVM_CREATE_METHOD_ID, evm_create_method_input);
        let evm_create_core_input_vec = evm_create_core_input.encode_to_vec(0);

        let mut runtime_ctx = RuntimeContext::<DefaultEmptyRuntimeDatabase>::new(ecl_rwasm.clone())
            .with_jzkt(LowLevelSDK::with_default_jzkt())
            .with_state(STATE_DEPLOY);
        let mut test_ctx = TestingContext::<IS_RUNTIME>::new();
        test_ctx
            .try_add_account(&caller_account)
            .contract_input_wrapper
            .set_journal_checkpoint(runtime_ctx.jzkt().checkpoint().to_u64())
            .set_contract_input(Bytes::copy_from_slice(&evm_create_core_input_vec))
            .set_block_chain_id(env_chain_id)
            .set_contract_caller(caller_address)
            .set_block_coinbase(block_coinbase)
            .set_tx_caller(caller_address);
        test_ctx.apply_ctx(&mut runtime_ctx);
        let jzkt = runtime_ctx.jzkt().clone();
        let output = test_ctx.run_rwasm_with_input(runtime_ctx, false, gas_limit);
        assert_eq!(ExitCode::Ok.into_i32(), output.exit_code);
        assert!(output.output.len() > 0);
        let contract_address = Address::from_slice(&output.output);
        assert_eq!(&expected_contract_address, &contract_address);

        (jzkt, contract_address)
    };

    {
        let evm_call_method_input = EvmCallMethodInput {
            callee: deployed_contract_address,
            value: call_value.into(),
            input: EVM_CONTRACT_BYTECODE1_METHOD_SAY_HELLO_WORLD_STR_ID.into(),
            gas_limit,
        };
        let evm_call_core_input = CoreInput::new(EVM_CALL_METHOD_ID, evm_call_method_input);
        let evm_call_core_input_vec = evm_call_core_input.encode_to_vec(0);

        let mut runtime_ctx = RuntimeContext::<DefaultEmptyRuntimeDatabase>::new(ecl_rwasm.clone())
            .with_jzkt(LowLevelSDK::with_default_jzkt())
            .with_jzkt(jzkt.clone());
        let mut test_ctx = TestingContext::<IS_RUNTIME>::new();
        test_ctx
            .contract_input_wrapper
            .set_journal_checkpoint(runtime_ctx.jzkt().checkpoint().to_u64())
            .set_contract_input(Bytes::copy_from_slice(&evm_call_core_input_vec))
            .set_contract_address(deployed_contract_address);
        test_ctx.apply_ctx(&mut runtime_ctx);
        let output_res = test_ctx.run_rwasm_with_input(runtime_ctx, false, gas_limit);
        assert_eq!(ExitCode::Ok.into_i32(), output_res.exit_code);
        let output = output_res.output;
        assert_eq!(
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 11, 72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ],
            output.as_slice(),
        );
    };
}

#[test]
fn test_evm_call_from_wasm() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_account = Account {
        address: caller_address,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };
    let gas_limit: u64 = 10_000_000;

    const IS_RUNTIME: bool = true;

    let jzkt = {
        let jzkt = LowLevelSDK::with_default_jzkt();
        let mut ecl_account = Account::new_from_jzkt(ECL_CONTRACT_ADDRESS);
        ecl_account.update_bytecode(
            &include_bytes!("../../../crates/contracts/assets/ecl_contract.wasm").into(),
            None,
            &include_bytes!("../../../crates/contracts/assets/ecl_contract.rwasm").into(),
            None,
        );
        ecl_account.write_to_jzkt();
        println!(
            "ecl_account.rwasm_bytecode_hash {}",
            ecl_account.rwasm_code_hash
        );
        Account::commit();
        jzkt
    };

    let (jzkt, deployed_contract_address) = {
        let expected_contract_address = calc_create_address(&caller_address, caller_account.nonce);
        let contract_input_code = EVM_CONTRACT_BYTECODE1;
        let create_value = B256::left_padding_from(&hex!("1000"));
        let evm_create_method_input = EvmCreateMethodInput {
            init_code: contract_input_code.into(),
            value: create_value.into(),
            gas_limit,
            salt: None,
        };
        let evm_create_core_input = CoreInput::new(EVM_CREATE_METHOD_ID, evm_create_method_input);
        let evm_create_core_input_vec = evm_create_core_input.encode_to_vec(0);
        let wasm_binary = include_bytes!("../../../crates/contracts/assets/ecl_contract.wasm");
        let rwasm_binary = wasm2rwasm(wasm_binary).unwrap();
        let mut runtime_ctx = RuntimeContext::new(rwasm_binary.clone())
            .with_state(STATE_MAIN)
            .with_jzkt(jzkt);
        let mut test_ctx = TestingContext::<IS_RUNTIME>::new();
        test_ctx
            .try_add_account(&caller_account)
            .contract_input_wrapper
            .set_journal_checkpoint(runtime_ctx.jzkt().checkpoint().to_u64())
            .set_contract_gas_limit(gas_limit.into())
            .set_contract_input(Bytes::copy_from_slice(&evm_create_core_input_vec))
            .set_contract_caller(caller_address);
        test_ctx.apply_ctx(&mut runtime_ctx);
        let jzkt = runtime_ctx.jzkt().clone();
        let output = test_ctx.run_rwasm_with_input(runtime_ctx, false, gas_limit);
        assert_eq!(ExitCode::Ok.into_i32(), output.exit_code);
        assert!(output.output.len() > 0);
        let evm_contract_address = Address::from_slice(&output.output);
        assert_eq!(&expected_contract_address, &evm_contract_address);

        (jzkt, evm_contract_address)
    };

    {
        let evm_call_from_wasm_wasm_binary =
            include_bytes!("../../../examples/bin/evm_call_from_wasm.wasm");
        let evm_call_from_wasm_rwasm_binary = wasm2rwasm(evm_call_from_wasm_wasm_binary).unwrap();

        let mut runtime_ctx = RuntimeContext::new(evm_call_from_wasm_rwasm_binary)
            .with_state(STATE_MAIN)
            .with_jzkt(jzkt.clone());
        let mut test_ctx = TestingContext::<IS_RUNTIME>::new();
        let contract_input = EVM_CONTRACT_BYTECODE1_METHOD_SAY_HELLO_WORLD_STR_ID;
        test_ctx
            .contract_input_wrapper
            .set_journal_checkpoint(runtime_ctx.jzkt().checkpoint().to_u64())
            .set_contract_gas_limit(gas_limit.into())
            .set_contract_input(contract_input.into())
            .set_contract_address(deployed_contract_address)
            .set_contract_caller(caller_address);
        test_ctx.apply_ctx(&mut runtime_ctx);
        let output = test_ctx.run_rwasm_with_input(runtime_ctx, false, gas_limit);
        assert_eq!(output.exit_code, ExitCode::Ok.into_i32());
        let call_output = output.output;
        assert_eq!(
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 11, 72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
            ],
            call_output.as_slice(),
        );
    }
}
