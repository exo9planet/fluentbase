use crate::{
    assets::evm_test_contract::{
        EVM_CONTRACT_BYTECODE1, EVM_CONTRACT_BYTECODE1_METHOD_GET_BALANCE_STR_ID,
        EVM_CONTRACT_BYTECODE1_METHOD_GET_SELF_BALANCE_STR_ID,
        EVM_CONTRACT_BYTECODE1_METHOD_SAY_HELLO_WORLD_STR_ID,
    },
    core::utils::TestingContext,
};
use fluentbase_core::{
    evm::{
        address::_evm_address, balance::_evm_balance, call::_evm_call, create::_evm_create,
        selfbalance::_evm_self_balance,
    },
    helpers::{calc_create2_address, calc_create_address},
    Account,
};
use fluentbase_sdk::{Bytes20, Bytes32, LowLevelAPI, LowLevelSDK};
use fluentbase_sdk::{EvmCallMethodInput, EvmCreateMethodInput};
use fluentbase_types::{address, Address, Bytes, B256, U256};
use revm_interpreter::primitives::{hex, Bytecode};

#[test]
fn create2_address_correctness_test() {
    let tests = [
        (
            "0000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "00",
            "4D1A2e2bB4F88F0250f26Ffff098B0b30B26BF38",
        ),
        (
            "deadbeef00000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "00",
            "B928f69Bb1D91Cd65274e3c79d8986362984fDA3",
        ),
        (
            "deadbeef00000000000000000000000000000000",
            "000000000000000000000000feed000000000000000000000000000000000000",
            "00",
            "D04116cDd17beBE565EB2422F2497E06cC1C9833",
        ),
        (
            "0000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "deadbeef",
            "70f2b2914A2a4b783FaEFb75f459A580616Fcb5e",
        ),
        (
            "00000000000000000000000000000000deadbeef",
            "00000000000000000000000000000000000000000000000000000000cafebabe",
            "deadbeef",
            "60f3f640a8508fC6a86d45DF051962668E1e8AC7",
        ),
        (
            "00000000000000000000000000000000deadbeef",
            "00000000000000000000000000000000000000000000000000000000cafebabe",
            "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
            "1d8bfDC5D46DC4f61D6b6115972536eBE6A8854C",
        ),
        (
            "0000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "",
            "E33C0C7F7df4809055C3ebA6c09CFe4BaF1BD9e0",
        ),
    ];
    for (from, salt, init_code, expected) in tests {
        let from = from.parse::<Address>().unwrap();

        let salt = hex::decode(salt).unwrap();
        let salt: [u8; 32] = salt.try_into().unwrap();

        let init_code = hex::decode(init_code).unwrap();
        let mut init_code_hash: B256 = B256::default();
        LowLevelSDK::crypto_keccak256(
            init_code.as_ptr(),
            init_code.len() as u32,
            init_code_hash.as_mut_ptr(),
        );

        let expected = expected.parse::<Address>().unwrap();

        assert_eq!(expected, from.create2(salt, init_code_hash));
        assert_eq!(expected, from.create2_from_code(salt, init_code));
    }
}

#[test]
fn evm_create_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let expected_contract_address = calc_create_address(&caller_address, caller_nonce);
    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_data_bytes = "some contract input".as_bytes();

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx.try_add_account(&caller_account);
    test_ctx
        .contract_input_wrapper
        .set_journal_checkpoint(LowLevelSDK::jzkt_checkpoint().into())
        .set_contract_input(Bytes::copy_from_slice(contract_input_data_bytes))
        .set_block_chain_id(env_chain_id)
        .set_contract_address(expected_contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let value = B256::left_padding_from(&hex!("1000"));
    let gas_limit: u64 = 10_000_000;
    let created_contract_address = _evm_create(EvmCreateMethodInput {
        init_code: EVM_CONTRACT_BYTECODE1.into(),
        value: value.into(),
        gas_limit,
        salt: None,
    })
    .unwrap();
    assert_eq!(expected_contract_address, created_contract_address);
}

#[test]
fn evm_call_after_create_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let computed_contract_address = calc_create_address(&caller_address, caller_nonce);
    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_data_bytes = "some contract input".as_bytes();

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx.try_add_account(&caller_account);
    test_ctx
        .contract_input_wrapper
        .set_contract_input(Bytes::copy_from_slice(contract_input_data_bytes))
        .set_block_chain_id(env_chain_id)
        .set_contract_address(computed_contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let create_value = U256::from_be_slice(&hex!("1000"));
    let gas_limit: u64 = 10_000_000;
    let created_address = _evm_create(EvmCreateMethodInput {
        init_code: EVM_CONTRACT_BYTECODE1.into(),
        value: create_value,
        gas_limit,
        salt: None,
    })
    .unwrap();
    assert_eq!(computed_contract_address, created_address);

    let args = Vec::from(EVM_CONTRACT_BYTECODE1_METHOD_SAY_HELLO_WORLD_STR_ID);
    let call_value = U256::from_be_slice(&hex!("00"));
    let call_result = _evm_call(EvmCallMethodInput {
        callee: created_address,
        value: call_value,
        input: args.into(),
        gas_limit,
    });
    if call_result.exit_code != 0 {
        panic!("call failed with exit code: {}", call_result.exit_code)
    }
    assert_eq!(
        call_result.output.as_ref(),
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 11, 72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn evm_call_after_create2_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let contract_bytecode_ = Bytecode::new_raw(Bytes::copy_from_slice(EVM_CONTRACT_BYTECODE1));
    let contract_bytecode_hash = B256::from_slice(contract_bytecode_.hash_slow().as_slice());
    let salt = B256::left_padding_from(hex!("bc162382638a").as_slice());
    let computed_contract_address =
        calc_create2_address(&caller_address, &salt.into(), &contract_bytecode_hash);
    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_data_bytes = "some contract input".as_bytes();

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx
        .try_add_account(&caller_account)
        .contract_input_wrapper
        .set_contract_input(Bytes::copy_from_slice(contract_input_data_bytes))
        .set_block_chain_id(env_chain_id)
        .set_contract_address(computed_contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let create_value = U256::from_be_slice(&hex!("1000"));
    let gas_limit: u64 = 10_000_000;
    let created_address = _evm_create(EvmCreateMethodInput {
        value: create_value,
        init_code: EVM_CONTRACT_BYTECODE1.into(),
        gas_limit,
        salt: Some(salt.into()),
    })
    .unwrap();
    assert_eq!(computed_contract_address, created_address);

    let args_data = Vec::from(EVM_CONTRACT_BYTECODE1_METHOD_SAY_HELLO_WORLD_STR_ID);
    let call_value = U256::from_be_slice(&hex!("00"));
    let return_data = _evm_call(EvmCallMethodInput {
        callee: created_address,
        value: call_value,
        input: args_data.into(),
        gas_limit,
    });
    if return_data.exit_code != 0 {
        panic!("call failed with exit code: {}", return_data.exit_code)
    }
    assert_eq!(
        return_data.output.as_ref(),
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 11, 72, 101, 108, 108, 111, 32, 87, 111, 114, 108, 100, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn evm_balance_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1234567u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let expected_contract_address = calc_create_address(&caller_address, caller_nonce);
    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx
        .try_add_account(&caller_account)
        .contract_input_wrapper
        .set_block_chain_id(env_chain_id)
        .set_contract_address(expected_contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let mut caller_balance_bytes32_fact = Bytes32::default();
    _evm_balance(
        caller_address.as_ptr(),
        caller_balance_bytes32_fact.as_mut_ptr(),
    );
    let caller_balance_fact = U256::from_le_slice(caller_balance_bytes32_fact.as_slice());
    assert_eq!(caller_account.balance, caller_balance_fact);
}

#[test]
fn evm_selfbalance_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1234567u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx
        .try_add_account(&caller_account)
        .contract_input_wrapper
        .set_block_chain_id(env_chain_id)
        .set_contract_address(caller_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let mut caller_balance_bytes32_fact = Bytes32::default();
    _evm_self_balance(caller_balance_bytes32_fact.as_mut_ptr());
    let caller_balance_fact = U256::from_le_slice(caller_balance_bytes32_fact.as_slice());
    assert_eq!(caller_account.balance, caller_balance_fact);
}

#[test]
fn evm_address_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let contract_address = address!("000000000000000000000000000000000000000b");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1234567u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx
        .try_add_account(&caller_account)
        .contract_input_wrapper
        .set_block_chain_id(env_chain_id)
        .set_contract_address(contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let mut address_bytes20_fact = Bytes20::default();
    _evm_address(address_bytes20_fact.as_mut_ptr());
    assert_eq!(contract_address.as_slice(), address_bytes20_fact.as_slice());
}

#[test]
fn evm_selfbalance_from_contract_call_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from_be_slice(1000000000u128.to_be_bytes().as_slice()),
        ..Default::default()
    };

    let computed_contract_address = calc_create_address(&caller_address, caller_nonce);
    let contract_value = U256::from_be_slice(&hex!("0123456789abcdef"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_data_bytes = "some contract input".as_bytes();

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx
        .try_add_account(&caller_account)
        .contract_input_wrapper
        .set_contract_input(Bytes::copy_from_slice(contract_input_data_bytes))
        .set_block_chain_id(env_chain_id)
        .set_contract_address(computed_contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let create_value_hex_bytes = hex!("1000");
    let create_value = U256::from_be_slice(create_value_hex_bytes.as_slice());
    let gas_limit: u64 = 10_000_000;
    let created_address = _evm_create(EvmCreateMethodInput {
        init_code: EVM_CONTRACT_BYTECODE1.into(),
        value: create_value,
        gas_limit,
        salt: None,
    })
    .unwrap();
    assert_eq!(computed_contract_address, created_address);
    let mut created_address_balance = U256::default();
    Account::jzkt_get_balance(created_address.into_word().as_ptr(), unsafe {
        created_address_balance.as_le_slice_mut().as_mut_ptr()
    });
    assert_eq!(create_value, created_address_balance);

    let args_data = EVM_CONTRACT_BYTECODE1_METHOD_GET_SELF_BALANCE_STR_ID.to_vec();
    let call_value = U256::from_be_slice(&hex!("00"));
    let return_data = _evm_call(EvmCallMethodInput {
        callee: created_address,
        value: call_value,
        input: args_data.into(),
        gas_limit,
    });
    let expected_return_data = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 4, 52, 48, 57, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(
        hex::encode(expected_return_data),
        hex::encode(return_data.output.as_ref())
    );
}

#[test]
fn evm_balance_from_contract_call_test() {
    let caller_address = address!("000000000000000000000000000000000000000c");
    let caller_nonce = 1;
    let caller_account = Account {
        address: caller_address,
        nonce: caller_nonce,
        balance: U256::from(432425321425u128),
        ..Default::default()
    };

    let computed_contract_address = calc_create_address(&caller_address, caller_nonce);
    let contract_value = U256::from_be_slice(&hex!("84326482"));
    let contract_is_static = false;
    let block_coinbase: Address = address!("0000000000000000000000000000000000000012");
    let env_chain_id = 1;

    let contract_input_data_bytes = "some contract input".as_bytes();

    let mut test_ctx = TestingContext::<false>::new();
    test_ctx.try_add_account(&caller_account);
    test_ctx
        .contract_input_wrapper
        .set_contract_input(Bytes::copy_from_slice(contract_input_data_bytes))
        .set_block_chain_id(env_chain_id)
        .set_contract_address(computed_contract_address)
        .set_contract_caller(caller_address)
        .set_contract_value(contract_value)
        .set_block_coinbase(block_coinbase)
        .set_tx_caller(caller_address)
        .set_contract_is_static(contract_is_static);
    test_ctx.apply_ctx();

    let create_value_hex_bytes = hex!("84326482");
    let create_value = U256::from_be_slice(&create_value_hex_bytes);
    let gas_limit: u64 = 10_000_000;
    let created_address = _evm_create(EvmCreateMethodInput {
        init_code: EVM_CONTRACT_BYTECODE1.into(),
        value: create_value,
        gas_limit,
        salt: None,
    })
    .unwrap();
    assert_eq!(
        hex::encode(computed_contract_address),
        hex::encode(created_address)
    );
    let mut created_address_balance = U256::default();
    Account::jzkt_get_balance(created_address.into_word().as_ptr(), unsafe {
        created_address_balance.as_le_slice_mut().as_mut_ptr()
    });
    assert_eq!(create_value, created_address_balance);

    let mut args_data = EVM_CONTRACT_BYTECODE1_METHOD_GET_BALANCE_STR_ID.to_vec();
    args_data.extend_from_slice(caller_address.into_word().as_slice());
    let call_value = U256::from_be_slice(&hex!("00"));
    let return_data = _evm_call(EvmCallMethodInput {
        callee: created_address,
        value: call_value,
        input: args_data.into(),
        gas_limit,
    });
    let expected_return_data = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 12, 52, 51, 48, 50, 48, 55, 52, 50, 54, 51, 56, 51, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(
        hex::encode(expected_return_data),
        hex::encode(return_data.output.as_ref())
    );
}
