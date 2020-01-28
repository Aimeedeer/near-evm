use std::sync::Arc;

use ethereum_types::{Address, U256};
use evm::Factory;
use vm::{ActionParams, ActionValue, CallType, Ext, GasLeft, Schedule};

use crate::evm_state::{EvmState, StateStore, SubState};
use crate::near_ext::NearExt;
use crate::utils;

pub fn deploy_code(state: &mut dyn EvmState, address: &Address, code: &Vec<u8>) {
    state.set_code(address, code);
}

pub fn call(
    state: &mut dyn EvmState,
    sender: &Address,            // TODO: change this all to address
    value: Option<U256>,
    call_stack_depth: usize,
    contract_address: &Address,
    input: &Vec<u8>,
) -> Option<GasLeft> {
    run_and_commit_if_success(
        state,
        sender,
        value,
        call_stack_depth,
        contract_address,
        contract_address,
        input,
        false,
    )
}

pub fn delegate_call(
    state: &mut dyn EvmState,
    sender: &Address,
    call_stack_depth: usize,
    context: &Address,
    delegee: &Address,
    input: &Vec<u8>,
) -> Option<GasLeft> {
    run_and_commit_if_success(state, sender, None, call_stack_depth, context, delegee, input, false)
}

pub fn static_call(
    state: &mut dyn EvmState,
    sender: &Address,
    call_stack_depth: usize,
    contract_address: &Address,
    input: &Vec<u8>,
) -> Option<GasLeft> {
    run_and_commit_if_success(
        state,
        sender,
        None,
        call_stack_depth,
        contract_address,
        contract_address,
        input,
        true,
    )
}

// TODO: maybe don't run static calls through here?
fn run_and_commit_if_success(
    state: &mut dyn EvmState,
    sender: &Address,
    value: Option<U256>,
    call_stack_depth: usize,
    state_address: &Address,
    code_address: &Address,
    input: &Vec<u8>,
    is_static: bool,
) -> Option<GasLeft> {
    // run the interpreter and
    let (result, state_updates) = run_against_state(
        state,
        sender,
        value,
        call_stack_depth,
        state_address,
        code_address,
        input,
        is_static,
    );

    // Don't apply changes from a static context (these _should_ error in the ext)
    if is_static {
        return result;
    }

    // Apply known gas amount changes (all reverts are NeedsReturn)
    // Apply NeedsReturn changes if apply_state
    // Return the result unmodified
    match result {
        Some(GasLeft::Known(_)) => {
            state.commit_changes(&state_updates.unwrap());
            result
        }
        Some(GasLeft::NeedsReturn {
            gas_left: _,
            data: _,
            apply_state,
        }) => {
            if apply_state {
                state.commit_changes(&state_updates.unwrap());
            }
            result
        }
        None => None,
    }
}

/// Runs the interpreter. Produces state diffs
fn run_against_state(
    state: &dyn EvmState,
    sender: &Address,
    value: Option<U256>,
    call_stack_depth: usize,
    state_address: &Address,
    code_address: &Address,
    input: &Vec<u8>,
    is_static: bool,
) -> (Option<GasLeft>, Option<StateStore>) {
    let startgas = 1_000_000_000;
    let code = state.code_at(code_address).expect("code does not exist");

    let mut store = StateStore::default();
    let mut sub_state = SubState::new(sender, &mut store, state);

    let mut params = ActionParams::default();

    params.call_type = CallType::None;
    params.code = Some(Arc::new(code));
    params.origin = utils::predecessor_as_eth();
    params.sender = *sender;
    params.gas = U256::from(startgas);
    params.data = Some(input.to_vec());
    if let Some(val) = value {
        params.value = ActionValue::Transfer(val)
    }
    let mut ext = NearExt::new(
        *state_address,
        &mut sub_state,
        call_stack_depth,
        is_static,
    );
    ext.info.gas_limit = U256::from(startgas);
    ext.schedule = Schedule::new_constantinople();

    let instance = Factory::default().create(params, ext.schedule(), ext.depth());

    // Run the code
    let result = instance.exec(&mut ext);

    (result.ok().unwrap().ok(), Some(store))
}
