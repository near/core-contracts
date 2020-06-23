mod utils;

use crate::utils::{is_pool_paused, reward_pool, POOL_ACCOUNT_ID};
use near_primitives::types::Balance;
use utils::{init_pool, ntoy, pool_account, wait_epoch, ExternalUser};

#[test]
fn multi_accounts_max_roundtrip() {
    struct AccountStake {
        pub account: ExternalUser,
        pub staked: Balance,
    };
    let initial_pool_balance = ntoy(100);
    let (ref mut runtime, ref root) = init_pool(initial_pool_balance);
    assert_eq!(
        pool_account(runtime).amount + pool_account(runtime).locked,
        initial_pool_balance
    );
    let mut accounts: Vec<AccountStake> = vec![];
    let mut to_spend = 1;
    let mut spent_total = 0;
    let mut acc_no = 0;
    loop {
        to_spend = to_spend * 2;

        acc_no += 1;
        let acc = if let Ok(acc) =
            root.create_external(runtime, format!("account_{}", acc_no), ntoy(30) + to_spend)
        {
            acc
        } else {
            break;
        };
        acc.pool_deposit(runtime, to_spend).unwrap();
        spent_total += to_spend;
        dbg!(spent_total);
        let pool_acc = runtime.view_account(&"pool".into()).unwrap();
        assert_eq!(
            pool_acc.amount + pool_acc.locked,
            initial_pool_balance + spent_total
        );
        acc.pool_stake(runtime, to_spend).unwrap();
        accounts.push(AccountStake {
            account: acc,
            staked: to_spend,
        });
    }

    for AccountStake { account, staked } in &accounts {
        account.pool_unstake(runtime, *staked).unwrap();
    }
    wait_epoch(runtime);
    runtime.produce_block().unwrap();
    for AccountStake { account, staked } in &accounts {
        account.pool_withdraw(runtime, *staked).unwrap();
        assert_eq!(
            account.account(runtime).amount,
            ntoy(30) + *staked,
            "Account: {:?}, staked: {:?}",
            account.account_id(),
            staked
        );
    }

    assert_eq!(
        pool_account(runtime).amount + pool_account(runtime).locked,
        initial_pool_balance
    );
}

#[test]
fn test_pause_resume() {
    let deposit_amount = ntoy(40);
    let (mut runtime, root) = init_pool(ntoy(100));
    let bob = root
        .create_external(&mut runtime, "bob".into(), ntoy(100))
        .unwrap();

    assert!(!is_pool_paused(&mut runtime));

    root.pool_pause(&mut runtime).unwrap();

    assert!(is_pool_paused(&mut runtime));

    for _ in 0..4 {
        wait_epoch(&mut runtime);
    }

    let mut pool = runtime.view_account(&POOL_ACCOUNT_ID.into()).unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    runtime.force_account_update(POOL_ACCOUNT_ID.into(), &pool);

    bob.pool_deposit(&mut runtime, deposit_amount).unwrap();

    let res = bob.get_account_unstaked_balance(&runtime);
    assert_eq!(res, deposit_amount);

    bob.pool_stake(&mut runtime, deposit_amount).unwrap();

    let res = bob.get_account_staked_balance(&runtime);
    assert_eq!(res, deposit_amount);

    for _ in 0..4 {
        wait_epoch(&mut runtime);
    }

    bob.pool_ping(&mut runtime).unwrap();

    assert_eq!(pool_account(&mut runtime).locked, 0);

    let res = bob.get_account_staked_balance(&runtime);
    assert_eq!(res, deposit_amount);

    root.pool_resume(&mut runtime).unwrap();

    assert!(!is_pool_paused(&mut runtime));

    assert_ne!(pool_account(&mut runtime).locked, 0);

    for _ in 0..4 {
        wait_epoch(&mut runtime);
        reward_pool(&mut runtime, ntoy(1));
    }

    bob.pool_ping(&mut runtime).unwrap();

    let res = bob.get_account_staked_balance(&runtime);
    assert!(res > deposit_amount);
}
