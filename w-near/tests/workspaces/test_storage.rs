use crate::utils::{create_user, init_w_near, wrap_near, LEGACY_BYTE_COST, STORAGE_BALANCE};
use near_primitives::views::FinalExecutionStatus;
use near_sdk::json_types::U128;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;

#[tokio::test]
async fn test_wrap_without_storage_deposit() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

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
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N") - STORAGE_BALANCE);

    Ok(())
}

#[tokio::test]
async fn test_close_account_empty_balance() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = alice
        .call(&worker, contract.id().clone(), "storage_unregister")
        .args_json((Option::<bool>::None,))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.json::<bool>()?);

    Ok(())
}

#[tokio::test]
async fn test_close_account_non_empty_balance() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let res = alice
        .call(&worker, contract.id().clone(), "storage_unregister")
        .args_json((Option::<bool>::None,))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(format!("{:?}", res.status.as_failure())
        .contains("Can't unregister the account with the positive balance without force"));

    let res = alice
        .call(&worker, contract.id().clone(), "storage_unregister")
        .args_json((Some(false),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(format!("{:?}", res.status.as_failure())
        .contains("Can't unregister the account with the positive balance without force"));

    let res = alice
        .call(&worker, contract.id().clone(), "storage_unregister")
        .args_json((Some(true),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(res.json::<bool>()?);

    Ok(())
}

#[tokio::test]
async fn test_disallow_non_zero_storage_withdrawal() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = alice
        .call(&worker, contract.id().clone(), "storage_withdraw")
        .args_json((U128::from(0),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    let res = alice
        .call(&worker, contract.id().clone(), "storage_withdraw")
        .args_json((U128::from(1),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(format!("{:?}", res.status.as_failure())
        .contains("The amount is greater than the available storage balance"));

    Ok(())
}

#[tokio::test]
async fn test_disallow_storage_withdrawal_for_non_registered() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

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

    let res = alice
        .call(&worker, contract.id().clone(), "storage_withdraw")
        .args_json((U128::from(0),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(format!("{:?}", res.status.as_failure()).contains("not registered"));

    Ok(())
}

#[tokio::test]
async fn test_double_storage_deposit() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = contract
        .call(&worker, "storage_deposit")
        .args_json((alice.id(), Option::<bool>::None))?
        .gas(300_000_000_000_000)
        .deposit(125 * LEGACY_BYTE_COST)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    Ok(())
}

#[tokio::test]
async fn test_insufficient_storage_deposit() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

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

    let res = contract
        .call(&worker, "storage_deposit")
        .args_json((alice.id(), Option::<bool>::None))?
        .gas(300_000_000_000_000)
        .deposit(125 * LEGACY_BYTE_COST - 1)
        .transact()
        .await?;
    assert!(format!("{:?}", res.status.as_failure())
        .contains("The attached deposit is less than the minimum storage balance"));

    Ok(())
}
