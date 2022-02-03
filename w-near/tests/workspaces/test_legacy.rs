use crate::utils::{create_user, init_legacy, legacy_register_user, wrap_near, STORAGE_BALANCE};
use near_primitives::views::FinalExecutionStatus;
use near_sdk::json_types::U128;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;

#[tokio::test]
async fn test_upgrade() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_legacy(&worker).await?;

    let alice = legacy_register_user("alice", &contract, &worker).await?;

    let legacy_storage_minimum_balance: U128 = contract
        .call(&worker, "storage_minimum_balance")
        .view()
        .await?
        .json()?;
    assert_eq!(legacy_storage_minimum_balance.0, STORAGE_BALANCE);

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N"));

    let contract = contract
        .as_account()
        .deploy(&worker, include_bytes!("../../res/w_near.wasm").to_vec())
        .await?
        .result;

    let storage_minimum_balance: U128 = contract
        .call(&worker, "storage_minimum_balance")
        .view()
        .await?
        .json()?;
    assert_eq!(storage_minimum_balance.0, STORAGE_BALANCE);

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N"));

    let bob = create_user("bob", &contract, &worker).await?;

    let res = wrap_near(&bob, &contract, &worker, parse_near!("1.5 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let bob_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((bob.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(bob_balance.0, parse_near!("1.5 N"));

    let res = alice
        .call(&worker, contract.id().clone(), "ft_transfer")
        .args_json((
            bob.id(),
            U128::from(parse_near!("0.5 N")),
            Option::<bool>::None,
        ))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    let bob_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((bob.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(bob_balance.0, parse_near!("2 N"));

    Ok(())
}

#[tokio::test]
async fn test_legacy_ft_transfer() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_legacy(&worker).await?;

    let alice = legacy_register_user("alice", &contract, &worker).await?;

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N"));

    let bob = legacy_register_user("bob", &contract, &worker).await?;

    let res = alice
        .call(&worker, contract.id().clone(), "ft_transfer")
        .args_json((
            bob.id(),
            U128::from(parse_near!("0.5 N")),
            Option::<bool>::None,
        ))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    let bob_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((bob.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(bob_balance.0, parse_near!("0.5 N"));

    Ok(())
}

#[tokio::test]
async fn test_legacy_wrap_fail() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_legacy(&worker).await?;

    // Create a user, but do not register them
    let res = contract
        .as_account()
        .create_subaccount(&worker, "alice")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?;
    assert!(matches!(
        res.details.status,
        FinalExecutionStatus::SuccessValue(_)
    ));
    let alice = res.result;

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::Failure(_)));

    Ok(())
}
