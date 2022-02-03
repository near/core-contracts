use crate::utils::{create_user, init_w_near, wrap_near};
use near_primitives::views::FinalExecutionStatus;
use near_sdk::json_types::U128;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;

#[tokio::test]
async fn test_total_supply() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let bob = create_user("bob", &contract, &worker).await?;

    let res = wrap_near(&bob, &contract, &worker, parse_near!("2 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let total_supply: U128 = contract
        .call(&worker, "ft_total_supply")
        .view()
        .await?
        .json()?;
    assert_eq!(total_supply.0, parse_near!("3 N"));

    Ok(())
}

#[tokio::test]
async fn test_withdraw_near() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N"));

    let res = alice
        .call(&worker, contract.id().clone(), "near_withdraw")
        .args_json((U128::from(parse_near!("0.5 N")),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("0.5 N"));

    Ok(())
}

#[tokio::test]
async fn test_withdraw_too_much_near() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;

    let alice = create_user("alice", &contract, &worker).await?;

    let res = wrap_near(&alice, &contract, &worker, parse_near!("1 N")).await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N"));

    let res = alice
        .call(&worker, contract.id().clone(), "near_withdraw")
        .args_json((U128::from(parse_near!("1.5 N")),))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::Failure(_)));

    let alice_balance: U128 = contract
        .call(&worker, "ft_balance_of")
        .args_json((alice.id(),))?
        .view()
        .await?
        .json()?;
    assert_eq!(alice_balance.0, parse_near!("1 N"));

    Ok(())
}
