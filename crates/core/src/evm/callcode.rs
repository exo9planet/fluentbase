use crate::{account::Account, fluent_host::FluentHost, helpers::DefaultEvmSpec};
use alloc::boxed::Box;
use core::ptr;
use fluentbase_sdk::{evm::ExecutionContext, LowLevelAPI, LowLevelSDK};
use fluentbase_types::{Address, ExitCode, U256};
use revm_interpreter::{
    analysis::to_analysed, opcode::make_instruction_table, primitives::Bytecode, BytecodeLocked,
    Contract, Interpreter, SharedMemory,
};

pub fn _evm_callcode(
    gas_limit: u32,
    callee_address20_offset: *const u8,
    value32_offset: *const u8,
    args_offset: *const u8,
    args_size: u32,
    ret_offset: *mut u8,
    ret_size: u32,
) -> ExitCode {
    let value = U256::from_be_slice(unsafe { &*ptr::slice_from_raw_parts(value32_offset, 32) });
    let callee_address =
        Address::from_slice(unsafe { &*ptr::slice_from_raw_parts(callee_address20_offset, 20) });
    let callee_account = Account::new_from_jzkt(callee_address);
    let caller_address = ExecutionContext::contract_caller();
    let caller_account = Account::new_from_jzkt(caller_address);
    if caller_account.balance < value {
        return ExitCode::InsufficientBalance;
    }
    // if value != U256::ZERO {
    //     match Account::transfer(&mut caller_account, &mut caller_account, value) {
    //         Ok(_) => {}
    //         Err(exit_code) => return exit_code,
    //     }
    // };
    let bytecode = BytecodeLocked::try_from(to_analysed(Bytecode::new_raw(
        callee_account.load_source_bytecode(),
    )))
    .unwrap();
    let contract = Contract {
        input: unsafe { &*ptr::slice_from_raw_parts(args_offset, args_size as usize) }.into(),
        hash: callee_account.source_code_hash,
        bytecode,
        address: caller_address,
        caller: caller_address,
        value,
    };
    let mut interpreter = Interpreter::new(
        Box::new(contract),
        gas_limit as u64,
        ExecutionContext::contract_is_static(),
    );
    let instruction_table = make_instruction_table::<FluentHost, DefaultEvmSpec>();
    let mut host = FluentHost::default();
    let shared_memory = SharedMemory::new();
    let result = match interpreter
        .run(shared_memory, &instruction_table, &mut host)
        .into_result_return()
    {
        Some(v) => v,
        None => return ExitCode::EVMCallError,
    };
    let exit_code = if result.is_error() {
        ExitCode::EVMCallError
    } else if result.is_revert() {
        ExitCode::EVMCallRevert
    } else {
        ExitCode::Ok
    };
    // write execution output
    let output = result.output;
    LowLevelSDK::sys_write(&output);
    if ret_size > 0 {
        let ret_size_actual = core::cmp::min(output.len(), ret_size as usize);
        unsafe { ptr::copy(output.as_ptr(), ret_offset, ret_size_actual) };
    }
    exit_code
}
