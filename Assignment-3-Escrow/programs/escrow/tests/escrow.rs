use std::path::Path;

use anchor_lang::{AccountDeserialize, InstructionData};
use litesvm::LiteSVM;
use solana_address::Address;
use solana_instruction::{account_meta::AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::Transaction;

use escrow::Escrow;

const MAKER_AIRDROP: u64 = 2_000_000_000;
const TAKER_AIRDROP: u64 = 2_000_000_000;

/// System program address (11111111111111111111111111111111)
const SYSTEM_PROGRAM: Address = Address::new_from_array([0u8; 32]);

/// Convert Anchor program ID (Pubkey) to solana_address::Address
fn program_id() -> Address {
    Address::from(escrow::ID.to_bytes())
}

/// Convert an Address to an Anchor Pubkey (for account deserialization comparisons)
fn to_pubkey(addr: &Address) -> anchor_lang::prelude::Pubkey {
    anchor_lang::prelude::Pubkey::new_from_array(addr.to_bytes())
}

fn load_program(svm: &mut LiteSVM) {
    let program_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/escrow.so");
    svm.add_program_from_file(program_id(), program_path)
        .expect("program load failed");
}

fn build_make_ix(maker: Address, escrow: Address, amount_a: u64, amount_b: u64) -> Instruction {
    let accounts = vec![
        AccountMeta::new(maker, true),
        AccountMeta::new(escrow, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: escrow::instruction::Make { amount_a, amount_b }.data(),
    }
}

fn build_take_ix(taker: Address, maker: Address, escrow: Address) -> Instruction {
    let accounts = vec![
        AccountMeta::new(taker, true),
        AccountMeta::new(maker, false),
        AccountMeta::new(escrow, false),
        AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: escrow::instruction::Take {}.data(),
    }
}

fn build_refund_ix(maker: Address, escrow: Address) -> Instruction {
    let accounts = vec![
        AccountMeta::new(maker, true),
        AccountMeta::new(escrow, false),
    ];

    Instruction {
        program_id: program_id(),
        accounts,
        data: escrow::instruction::Refund {}.data(),
    }
}

fn new_svm_with_program() -> LiteSVM {
    let mut svm = LiteSVM::new();
    load_program(&mut svm);
    svm
}

fn derive_escrow_pda(maker: &Address) -> (Address, u8) {
    Address::find_program_address(&[b"escrow", maker.as_ref()], &program_id())
}

// ─── Happy-path tests ───────────────────────────────────────────────────────

#[test]
fn test_make() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();
    let taker = Keypair::new();

    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();
    svm.airdrop(&taker.pubkey(), TAKER_AIRDROP).unwrap();

    let (escrow_pda, _bump) = derive_escrow_pda(&maker.pubkey());
    let amount_a = 400_000_000;
    let amount_b = 600_000_000;

    let ix = build_make_ix(maker.pubkey(), escrow_pda, amount_a, amount_b);
    let msg = Message::new(&[ix], Some(&maker.pubkey()));
    let tx = Transaction::new(&[&maker], msg, svm.latest_blockhash());
    svm.send_transaction(tx).unwrap();

    // Verify escrow state
    let escrow_account = svm.get_account(&escrow_pda).expect("escrow account missing");
    let mut data = escrow_account.data.as_slice();
    let state = Escrow::try_deserialize(&mut data).expect("deserialize failed");

    assert_eq!(state.maker, to_pubkey(&maker.pubkey()));
    assert_eq!(state.amount_a, amount_a);
    assert_eq!(state.amount_b, amount_b);

    // Verify lamports were transferred to escrow
    let escrow_lamports = svm.get_account(&escrow_pda).unwrap().lamports;
    assert!(escrow_lamports >= amount_a, "escrow should hold at least amount_a lamports");
}

#[test]
fn test_take() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();
    let taker = Keypair::new();

    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();
    svm.airdrop(&taker.pubkey(), TAKER_AIRDROP).unwrap();

    let (escrow_pda, _bump) = derive_escrow_pda(&maker.pubkey());
    let amount_a = 300_000_000;
    let amount_b = 500_000_000;

    // --- Make ---
    let make_ix = build_make_ix(maker.pubkey(), escrow_pda, amount_a, amount_b);
    let make_msg = Message::new(&[make_ix], Some(&maker.pubkey()));
    let make_tx = Transaction::new(&[&maker], make_msg, svm.latest_blockhash());
    svm.send_transaction(make_tx).unwrap();

    let maker_before = svm.get_account(&maker.pubkey()).unwrap().lamports;
    let taker_before = svm.get_account(&taker.pubkey()).unwrap().lamports;

    // --- Take ---
    let take_ix = build_take_ix(taker.pubkey(), maker.pubkey(), escrow_pda);
    let take_msg = Message::new(&[take_ix], Some(&taker.pubkey()));
    let take_tx = Transaction::new(&[&taker], take_msg, svm.latest_blockhash());
    svm.send_transaction(take_tx).unwrap();

    let maker_after = svm.get_account(&maker.pubkey()).unwrap().lamports;
    let taker_after = svm.get_account(&taker.pubkey()).unwrap().lamports;

    // Maker receives amount_b from taker + rent refund from closed escrow
    assert!(maker_after >= maker_before + amount_b);
    // Taker pays amount_b but receives amount_a (net = amount_a - amount_b - tx fee)
    assert!(taker_after > taker_before - amount_b);
    // Escrow account should be closed
    assert!(svm.get_account(&escrow_pda).is_none());
}

#[test]
fn test_refund() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();

    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();

    let (escrow_pda, _bump) = derive_escrow_pda(&maker.pubkey());
    let amount_a = 250_000_000;
    let amount_b = 350_000_000;

    // --- Make ---
    let make_ix = build_make_ix(maker.pubkey(), escrow_pda, amount_a, amount_b);
    let make_msg = Message::new(&[make_ix], Some(&maker.pubkey()));
    let make_tx = Transaction::new(&[&maker], make_msg, svm.latest_blockhash());
    svm.send_transaction(make_tx).unwrap();

    let maker_before = svm.get_account(&maker.pubkey()).unwrap().lamports;

    // --- Refund ---
    let refund_ix = build_refund_ix(maker.pubkey(), escrow_pda);
    let refund_msg = Message::new(&[refund_ix], Some(&maker.pubkey()));
    let refund_tx = Transaction::new(&[&maker], refund_msg, svm.latest_blockhash());
    svm.send_transaction(refund_tx).unwrap();

    let maker_after = svm.get_account(&maker.pubkey()).unwrap().lamports;

    // Maker gets back amount_a + rent from closed escrow (minus tx fee)
    assert!(maker_after >= maker_before + amount_a);
    // Escrow account should be closed
    assert!(svm.get_account(&escrow_pda).is_none());
}

// ─── Edge-case / failure tests ──────────────────────────────────────────────

#[test]
fn test_make_zero_amount_a_fails() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();

    let (escrow_pda, _) = derive_escrow_pda(&maker.pubkey());

    let ix = build_make_ix(maker.pubkey(), escrow_pda, 0, 500_000_000);
    let msg = Message::new(&[ix], Some(&maker.pubkey()));
    let tx = Transaction::new(&[&maker], msg, svm.latest_blockhash());
    let result = svm.send_transaction(tx);

    assert!(result.is_err(), "make with amount_a=0 should fail");
}

#[test]
fn test_make_zero_amount_b_fails() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();
    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();

    let (escrow_pda, _) = derive_escrow_pda(&maker.pubkey());

    let ix = build_make_ix(maker.pubkey(), escrow_pda, 500_000_000, 0);
    let msg = Message::new(&[ix], Some(&maker.pubkey()));
    let tx = Transaction::new(&[&maker], msg, svm.latest_blockhash());
    let result = svm.send_transaction(tx);

    assert!(result.is_err(), "make with amount_b=0 should fail");
}

#[test]
fn test_refund_by_non_maker_fails() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();
    let impostor = Keypair::new();

    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();
    svm.airdrop(&impostor.pubkey(), MAKER_AIRDROP).unwrap();

    let (escrow_pda, _) = derive_escrow_pda(&maker.pubkey());
    let amount_a = 200_000_000;
    let amount_b = 300_000_000;

    // Maker creates escrow
    let make_ix = build_make_ix(maker.pubkey(), escrow_pda, amount_a, amount_b);
    let make_msg = Message::new(&[make_ix], Some(&maker.pubkey()));
    let make_tx = Transaction::new(&[&maker], make_msg, svm.latest_blockhash());
    svm.send_transaction(make_tx).unwrap();

    // Impostor tries to refund — PDA seeds won't match
    let (impostor_escrow_pda, _) = derive_escrow_pda(&impostor.pubkey());
    let refund_ix = build_refund_ix(impostor.pubkey(), impostor_escrow_pda);
    let refund_msg = Message::new(&[refund_ix], Some(&impostor.pubkey()));
    let refund_tx = Transaction::new(&[&impostor], refund_msg, svm.latest_blockhash());
    let result = svm.send_transaction(refund_tx);

    assert!(result.is_err(), "non-maker should not be able to refund");
    // Original escrow should still exist
    assert!(svm.get_account(&escrow_pda).is_some());
}

#[test]
fn test_duplicate_make_fails() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();

    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();

    let (escrow_pda, _) = derive_escrow_pda(&maker.pubkey());

    // First make succeeds
    let ix1 = build_make_ix(maker.pubkey(), escrow_pda, 100_000_000, 200_000_000);
    let msg1 = Message::new(&[ix1], Some(&maker.pubkey()));
    let tx1 = Transaction::new(&[&maker], msg1, svm.latest_blockhash());
    svm.send_transaction(tx1).unwrap();

    // Second make should fail (PDA already initialized)
    let ix2 = build_make_ix(maker.pubkey(), escrow_pda, 100_000_000, 200_000_000);
    let msg2 = Message::new(&[ix2], Some(&maker.pubkey()));
    let tx2 = Transaction::new(&[&maker], msg2, svm.latest_blockhash());
    let result = svm.send_transaction(tx2);

    assert!(result.is_err(), "duplicate make should fail because PDA is already initialized");
}

#[test]
fn test_full_lifecycle_make_then_refund_then_make_again() {
    let mut svm = new_svm_with_program();
    let maker = Keypair::new();

    svm.airdrop(&maker.pubkey(), MAKER_AIRDROP).unwrap();

    let (escrow_pda, _) = derive_escrow_pda(&maker.pubkey());

    // Make
    let ix1 = build_make_ix(maker.pubkey(), escrow_pda, 100_000_000, 200_000_000);
    let msg1 = Message::new(&[ix1], Some(&maker.pubkey()));
    let tx1 = Transaction::new(&[&maker], msg1, svm.latest_blockhash());
    svm.send_transaction(tx1).unwrap();
    assert!(svm.get_account(&escrow_pda).is_some());

    // Refund
    let ix2 = build_refund_ix(maker.pubkey(), escrow_pda);
    let msg2 = Message::new(&[ix2], Some(&maker.pubkey()));
    let tx2 = Transaction::new(&[&maker], msg2, svm.latest_blockhash());
    svm.send_transaction(tx2).unwrap();
    assert!(svm.get_account(&escrow_pda).is_none());

    // Make again with different amounts — should succeed since PDA was closed
    let ix3 = build_make_ix(maker.pubkey(), escrow_pda, 150_000_000, 250_000_000);
    let msg3 = Message::new(&[ix3], Some(&maker.pubkey()));
    let tx3 = Transaction::new(&[&maker], msg3, svm.latest_blockhash());
    svm.send_transaction(tx3).unwrap();

    let escrow_account = svm.get_account(&escrow_pda).expect("escrow should exist again");
    let mut data = escrow_account.data.as_slice();
    let state = Escrow::try_deserialize(&mut data).unwrap();
    assert_eq!(state.amount_a, 150_000_000);
    assert_eq!(state.amount_b, 250_000_000);
}
