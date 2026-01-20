use litesvm::LiteSVM;
use litesvm_token::{get_spl_account, CreateAssociatedTokenAccount, CreateMint, MintTo, TOKEN_ID};

use solana_sdk::{
    message::{AccountMeta, Instruction},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use solana_system_interface::program;
use spl_associated_token_account::{
    get_associated_token_address, ID as ASSOCIATED_TOKEN_PROGRAM_ID,
};
use spl_token::{native_mint::DECIMALS, state::Account as TokenAccount};

#[test]
pub fn test_escrow() {
    let mut svm = LiteSVM::new();

    let payer = Keypair::new();
    let maker = Keypair::new();
    let taker = Keypair::new();

    svm.airdrop(&payer.pubkey(), 3 * LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&maker.pubkey(), 3 * LAMPORTS_PER_SOL).unwrap();
    svm.airdrop(&taker.pubkey(), 3 * LAMPORTS_PER_SOL).unwrap();

    let mint_a = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(DECIMALS)
        .send()
        .expect("failed to create mint");

    let mint_b = CreateMint::new(&mut svm, &payer)
        .authority(&payer.pubkey())
        .decimals(DECIMALS)
        .send()
        .expect("failed to create mint");

    let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
        .owner(&maker.pubkey())
        .send()
        .expect("failed to create ATA");

    let maker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_b)
        .owner(&maker.pubkey())
        .send()
        .expect("failed to create ATA");

    let taker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_a)
        .owner(&taker.pubkey())
        .send()
        .expect("failed to create ATA");

    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_b)
        .owner(&taker.pubkey())
        .send()
        .expect("failed to create ATA");

    MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, 1000)
        .owner(&payer)
        .send()
        .expect("Failed to mint");

    MintTo::new(&mut svm, &taker, &mint_b, &taker_ata_b, 1000)
        .owner(&payer)
        .send()
        .expect("Failed to mint");

    let program_id_bytes: [u8; 32] = [
        0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb,
        0xee, 0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc,
        0x11, 0xf7,
    ];
    let program_id = Pubkey::from(program_id_bytes);
    let program_bytes = include_bytes!("../../target/deploy/escrow_pinocchio.so");

    svm.add_program(program_id, program_bytes)
        .expect("failed to add program");

    let seed: u64 = 1;

    let (escrow, _) = Pubkey::find_program_address(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );

    let vault = get_associated_token_address(&escrow, &mint_a);

    // Make escrow
    let receive: u64 = 200;
    let amount: u64 = 500;

    let mut ix_data = vec![];
    ix_data.push(0);
    ix_data.extend_from_slice(&seed.to_le_bytes());
    ix_data.extend_from_slice(&receive.to_le_bytes());
    ix_data.extend_from_slice(&amount.to_le_bytes());

    let instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new(escrow, false),
            AccountMeta::new(mint_a, false),
            AccountMeta::new(mint_b, false),
            AccountMeta::new(maker_ata_a, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(program::ID, false),
            AccountMeta::new_readonly(TOKEN_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false), // Add this!
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&maker.pubkey()),
        &[&maker],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);

    if let Ok(response) = &result {
        let logs = response.pretty_logs();
        println!("Transaction logs:\n{}", logs);
    } else if let Err(e) = &result {
        // Try to get logs from error if available
        eprintln!("Transaction failed: {:?}", e);
    }

    let maker_account: TokenAccount = get_spl_account(&svm, &maker_ata_a).unwrap();
    let vault_account: TokenAccount = get_spl_account(&svm, &vault).unwrap();

    assert_eq!(maker_account.amount, 500);
    assert_eq!(vault_account.amount, 500);

    // Refund and close escrow
    let ix_data = vec![1];

    let instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(taker.pubkey(), true),
            AccountMeta::new(maker.pubkey(), true),
            AccountMeta::new(escrow, false),
            AccountMeta::new(mint_a, false),
            AccountMeta::new(mint_b, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(taker_ata_a, false),
            AccountMeta::new(taker_ata_b, false),
            AccountMeta::new(maker_ata_b, false),
            AccountMeta::new_readonly(program::ID, false),
            AccountMeta::new_readonly(TOKEN_ID, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&maker.pubkey()),
        &[&maker, &taker],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);

    if let Ok(response) = &result {
        let logs = response.pretty_logs();
        println!("Transaction logs:\n{}", logs);
    } else if let Err(e) = &result {
        // Try to get logs from error if available
        eprintln!("Transaction failed: {:?}", e);
    }

    let maker_token_account_a: TokenAccount = get_spl_account(&svm, &maker_ata_a).unwrap();
    let vault_account = get_spl_account::<TokenAccount>(&svm, &vault);

    assert!(vault_account.is_err());
    assert_eq!(maker_token_account_a.amount, 500);

    // Refund and close escrow
    // let ix_data = vec![2];

    // let instruction = Instruction {
    //     program_id,
    //     accounts: vec![
    //         AccountMeta::new(maker.pubkey(), true),
    //         AccountMeta::new(escrow, false),
    //         AccountMeta::new(mint_a, false),
    //         AccountMeta::new(vault, false),
    //         AccountMeta::new(maker_ata_a, false),
    //         AccountMeta::new_readonly(program::ID, false),
    //         AccountMeta::new_readonly(TOKEN_ID, false),
    //         AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
    //     ],
    //     data: ix_data,
    // };

    // let tx = Transaction::new_signed_with_payer(
    //     &[instruction],
    //     Some(&maker.pubkey()),
    //     &[&maker],
    //     svm.latest_blockhash(),
    // );

    // let result = svm.send_transaction(tx);

    // if let Ok(response) = &result {
    //     let logs = response.pretty_logs();
    //     println!("Transaction logs:\n{}", logs);
    // } else if let Err(e) = &result {
    //     // Try to get logs from error if available
    //     eprintln!("Transaction failed: {:?}", e);
    // }

    // let maker_account: TokenAccount = get_spl_account(&svm, &maker_ata_a).unwrap();
    // let vault_account = get_spl_account::<TokenAccount>(&svm, &vault);

    // assert!(vault_account.is_err());
    // assert_eq!(maker_account.amount, 1000);
}
