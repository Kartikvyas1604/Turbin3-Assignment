use std::path::Path;

use anchor_lang::{AccountDeserialize, InstructionData};
use litesvm::LiteSVM;
use solana_address::Address;
use solana_instruction::{account_meta::AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

use amm::{LpAccount, PoolState};

const USER_AIRDROP: u64 = 3_000_000_000;
const SYSTEM_PROGRAM: Address = Address::new_from_array([0u8; 32]);

fn program_id() -> Address {
    Address::from(amm::ID.to_bytes())
}

fn to_pubkey(addr: &Address) -> anchor_lang::prelude::Pubkey {
    anchor_lang::prelude::Pubkey::new_from_array(addr.to_bytes())
}

fn load_program(svm: &mut LiteSVM) {
    let program_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/amm.so");
    svm.add_program_from_file(program_id(), program_path)
        .expect("program load failed");
}

fn new_svm_with_program() -> LiteSVM {
    let mut svm = LiteSVM::new();
    load_program(&mut svm);
    svm
}

fn derive_pool_pda(authority: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"pool", authority.as_ref()], &program_id())
}

fn derive_vault_a_pda(pool: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"vault_a", pool.as_ref()], &program_id())
}

fn derive_vault_b_pda(pool: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"vault_b", pool.as_ref()], &program_id())
}

fn derive_lp_pda(pool: &Address, user: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"lp", pool.as_ref(), user.as_ref()], &program_id())
}

fn build_initialize_ix(
    authority: Address,
    pool: Address,
    vault_a: Address,
    vault_b: Address,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(authority, true),
        AccountMeta::new(pool, false),
        AccountMeta::new(vault_a, false),
        AccountMeta::new(vault_b, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: amm::instruction::InitializePool {}.data(),
    }
}

fn build_add_liquidity_ix(
    user: Address,
    pool: Address,
    vault_a: Address,
    vault_b: Address,
    lp_account: Address,
    amount_a: u64,
    amount_b: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(user, true),
        AccountMeta::new(pool, false),
        AccountMeta::new(vault_a, false),
        AccountMeta::new(vault_b, false),
        AccountMeta::new(lp_account, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: amm::instruction::AddLiquidity { amount_a, amount_b }.data(),
    }
}

fn build_remove_liquidity_ix(
    user: Address,
    pool: Address,
    vault_a: Address,
    vault_b: Address,
    lp_account: Address,
    lp_amount: u64,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(user, true),
        AccountMeta::new(pool, false),
        AccountMeta::new(vault_a, false),
        AccountMeta::new(vault_b, false),
        AccountMeta::new(lp_account, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: amm::instruction::RemoveLiquidity { lp_amount }.data(),
    }
}

fn build_swap_ix(
    user: Address,
    pool: Address,
    vault_a: Address,
    vault_b: Address,
    amount_in: u64,
    min_out: u64,
    a_to_b: bool,
) -> Instruction {
    let accounts = vec![
        AccountMeta::new(user, true),
        AccountMeta::new(pool, false),
        AccountMeta::new(vault_a, false),
        AccountMeta::new(vault_b, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: amm::instruction::Swap {
            amount_in,
            min_out,
            a_to_b,
        }
        .data(),
    }
}

fn get_amount_out(amount_in: u64, reserve_in: u64, reserve_out: u64) -> u64 {
    let amount_in = amount_in as u128;
    let reserve_in = reserve_in as u128;
    let reserve_out = reserve_out as u128;

    let amount_in_with_fee = amount_in * 997 / 1000;
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in + amount_in_with_fee;

    (numerator / denominator) as u64
}

#[test]
fn test_initialize_pool() {
    let mut svm = new_svm_with_program();
    let authority = Keypair::new();

    svm.airdrop(&authority.pubkey(), USER_AIRDROP).unwrap();

    let (pool_pda, _) = derive_pool_pda(&authority.pubkey());
    let (vault_a_pda, _) = derive_vault_a_pda(&pool_pda);
    let (vault_b_pda, _) = derive_vault_b_pda(&pool_pda);

    let ix = build_initialize_ix(authority.pubkey(), pool_pda, vault_a_pda, vault_b_pda);
    let msg = Message::new(&[ix], Some(&authority.pubkey()));
    let tx = Transaction::new(&[&authority], msg, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();

    let pool_account = svm.get_account(&pool_pda).expect("pool missing");
    let mut data = pool_account.data.as_slice();
    let state = PoolState::try_deserialize(&mut data).expect("deserialize failed");

    assert_eq!(state.authority, to_pubkey(&authority.pubkey()));
    assert_eq!(state.vault_a, to_pubkey(&vault_a_pda));
    assert_eq!(state.vault_b, to_pubkey(&vault_b_pda));
    assert_eq!(state.reserve_a, 0);
    assert_eq!(state.reserve_b, 0);
    assert_eq!(state.total_lp, 0);

    assert!(svm.get_account(&vault_a_pda).is_some());
    assert!(svm.get_account(&vault_b_pda).is_some());
}

#[test]
fn test_add_liquidity() {
    let mut svm = new_svm_with_program();
    let authority = Keypair::new();

    svm.airdrop(&authority.pubkey(), USER_AIRDROP).unwrap();

    let (pool_pda, _) = derive_pool_pda(&authority.pubkey());
    let (vault_a_pda, _) = derive_vault_a_pda(&pool_pda);
    let (vault_b_pda, _) = derive_vault_b_pda(&pool_pda);
    let (lp_pda, _) = derive_lp_pda(&pool_pda, &authority.pubkey());

    let init_ix = build_initialize_ix(authority.pubkey(), pool_pda, vault_a_pda, vault_b_pda);
    let init_msg = Message::new(&[init_ix], Some(&authority.pubkey()));
    let init_tx = Transaction::new(&[&authority], init_msg, svm.latest_blockhash());
    svm.send_transaction(init_tx).unwrap();

    let amount_a = 400_000_000;
    let amount_b = 400_000_000;

    let add_ix = build_add_liquidity_ix(
        authority.pubkey(),
        pool_pda,
        vault_a_pda,
        vault_b_pda,
        lp_pda,
        amount_a,
        amount_b,
    );
    let add_msg = Message::new(&[add_ix], Some(&authority.pubkey()));
    let add_tx = Transaction::new(&[&authority], add_msg, svm.latest_blockhash());
    svm.send_transaction(add_tx).unwrap();

    let pool_account = svm.get_account(&pool_pda).expect("pool missing");
    let mut data = pool_account.data.as_slice();
    let state = PoolState::try_deserialize(&mut data).expect("deserialize failed");

    let lp_account = svm.get_account(&lp_pda).expect("lp missing");
    let mut lp_data = lp_account.data.as_slice();
    let lp_state = LpAccount::try_deserialize(&mut lp_data).expect("deserialize failed");

    assert_eq!(state.reserve_a, amount_a);
    assert_eq!(state.reserve_b, amount_b);
    assert_eq!(state.total_lp, lp_state.amount);
    assert_eq!(lp_state.owner, to_pubkey(&authority.pubkey()));
    assert_eq!(lp_state.pool, to_pubkey(&pool_pda));

    let vault_a_balance = svm.get_account(&vault_a_pda).unwrap().lamports;
    let vault_b_balance = svm.get_account(&vault_b_pda).unwrap().lamports;

    assert!(vault_a_balance >= amount_a);
    assert!(vault_b_balance >= amount_b);
}

#[test]
fn test_remove_liquidity() {
    let mut svm = new_svm_with_program();
    let authority = Keypair::new();

    svm.airdrop(&authority.pubkey(), USER_AIRDROP).unwrap();

    let (pool_pda, _) = derive_pool_pda(&authority.pubkey());
    let (vault_a_pda, _) = derive_vault_a_pda(&pool_pda);
    let (vault_b_pda, _) = derive_vault_b_pda(&pool_pda);
    let (lp_pda, _) = derive_lp_pda(&pool_pda, &authority.pubkey());

    let init_ix = build_initialize_ix(authority.pubkey(), pool_pda, vault_a_pda, vault_b_pda);
    let init_msg = Message::new(&[init_ix], Some(&authority.pubkey()));
    let init_tx = Transaction::new(&[&authority], init_msg, svm.latest_blockhash());
    svm.send_transaction(init_tx).unwrap();

    let amount_a = 500_000_000;
    let amount_b = 500_000_000;

    let add_ix = build_add_liquidity_ix(
        authority.pubkey(),
        pool_pda,
        vault_a_pda,
        vault_b_pda,
        lp_pda,
        amount_a,
        amount_b,
    );
    let add_msg = Message::new(&[add_ix], Some(&authority.pubkey()));
    let add_tx = Transaction::new(&[&authority], add_msg, svm.latest_blockhash());
    svm.send_transaction(add_tx).unwrap();

    let pool_before = svm.get_account(&pool_pda).unwrap();
    let mut pool_data = pool_before.data.as_slice();
    let pool_state = PoolState::try_deserialize(&mut pool_data).unwrap();

    let lp_before = svm.get_account(&lp_pda).unwrap();
    let mut lp_data = lp_before.data.as_slice();
    let lp_state = LpAccount::try_deserialize(&mut lp_data).unwrap();

    let lp_amount = lp_state.amount / 2;
    let user_before = svm.get_account(&authority.pubkey()).unwrap().lamports;

    let remove_ix = build_remove_liquidity_ix(
        authority.pubkey(),
        pool_pda,
        vault_a_pda,
        vault_b_pda,
        lp_pda,
        lp_amount,
    );
    let remove_msg = Message::new(&[remove_ix], Some(&authority.pubkey()));
    let remove_tx = Transaction::new(&[&authority], remove_msg, svm.latest_blockhash());
    svm.send_transaction(remove_tx).unwrap();

    let pool_after = svm.get_account(&pool_pda).unwrap();
    let mut pool_after_data = pool_after.data.as_slice();
    let pool_state_after = PoolState::try_deserialize(&mut pool_after_data).unwrap();

    let lp_after = svm.get_account(&lp_pda).unwrap();
    let mut lp_after_data = lp_after.data.as_slice();
    let lp_state_after = LpAccount::try_deserialize(&mut lp_after_data).unwrap();

    let user_after = svm.get_account(&authority.pubkey()).unwrap().lamports;

    assert_eq!(pool_state_after.total_lp, pool_state.total_lp - lp_amount);
    assert_eq!(lp_state_after.amount, lp_state.amount - lp_amount);
    assert!(user_after > user_before);
}

#[test]
fn test_swap_a_to_b() {
    let mut svm = new_svm_with_program();
    let authority = Keypair::new();

    svm.airdrop(&authority.pubkey(), USER_AIRDROP).unwrap();

    let (pool_pda, _) = derive_pool_pda(&authority.pubkey());
    let (vault_a_pda, _) = derive_vault_a_pda(&pool_pda);
    let (vault_b_pda, _) = derive_vault_b_pda(&pool_pda);
    let (lp_pda, _) = derive_lp_pda(&pool_pda, &authority.pubkey());

    let init_ix = build_initialize_ix(authority.pubkey(), pool_pda, vault_a_pda, vault_b_pda);
    let init_msg = Message::new(&[init_ix], Some(&authority.pubkey()));
    let init_tx = Transaction::new(&[&authority], init_msg, svm.latest_blockhash());
    svm.send_transaction(init_tx).unwrap();

    let amount_a = 700_000_000;
    let amount_b = 700_000_000;

    let add_ix = build_add_liquidity_ix(
        authority.pubkey(),
        pool_pda,
        vault_a_pda,
        vault_b_pda,
        lp_pda,
        amount_a,
        amount_b,
    );
    let add_msg = Message::new(&[add_ix], Some(&authority.pubkey()));
    let add_tx = Transaction::new(&[&authority], add_msg, svm.latest_blockhash());
    svm.send_transaction(add_tx).unwrap();

    let pool_before = svm.get_account(&pool_pda).unwrap();
    let mut pool_data = pool_before.data.as_slice();
    let pool_state = PoolState::try_deserialize(&mut pool_data).unwrap();

    let amount_in = 100_000_000;
    let expected_out = get_amount_out(amount_in, pool_state.reserve_a, pool_state.reserve_b);

    let swap_ix = build_swap_ix(
        authority.pubkey(),
        pool_pda,
        vault_a_pda,
        vault_b_pda,
        amount_in,
        1,
        true,
    );
    let swap_msg = Message::new(&[swap_ix], Some(&authority.pubkey()));
    let swap_tx = Transaction::new(&[&authority], swap_msg, svm.latest_blockhash());
    svm.send_transaction(swap_tx).unwrap();

    let pool_after = svm.get_account(&pool_pda).unwrap();
    let mut pool_after_data = pool_after.data.as_slice();
    let pool_state_after = PoolState::try_deserialize(&mut pool_after_data).unwrap();

    assert_eq!(pool_state_after.reserve_a, pool_state.reserve_a + amount_in);
    assert_eq!(pool_state_after.reserve_b, pool_state.reserve_b - expected_out);
}
