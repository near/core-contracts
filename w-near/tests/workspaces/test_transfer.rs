use crate::utils::{create_user, init_defi, init_w_near, register_user, wrap_near};
use near_primitives::views::FinalExecutionStatus;
use near_sdk::json_types::U128;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;

#[tokio::test]
async fn test_ft_transfer() -> anyhow::Result<()> {
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

    let bob = create_user("bob", &contract, &worker).await?;

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
async fn test_transfer_call_invest() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;
    let defi_contract = init_defi(&worker, &contract).await?;

    // register root & defi accounts as wNEAR accounts
    register_user(&contract.as_account(), &contract, &worker).await?;
    register_user(&defi_contract.as_account(), &contract, &worker).await?;

    let res = wrap_near(
        contract.as_account(),
        &contract,
        &worker,
        parse_near!("3 N"),
    )
    .await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    // root invests in defi by calling `ft_transfer_call`
    let res = contract
        .call(&worker, "ft_transfer_call")
        .args_json((
            defi_contract.id(),
            U128::from(parse_near!("1 N")),
            Option::<String>::None,
            "invest",
        ))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    let root_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    let defi_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((defi_contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(root_balance.0, parse_near!("2 N"));
    assert_eq!(defi_balance.0, parse_near!("1 N"));

    Ok(())
}

#[tokio::test]
async fn test_transfer_call_when_called_contract_not_registered_with_ft() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;
    let defi_contract = init_defi(&worker, &contract).await?;

    register_user(&contract.as_account(), &contract, &worker).await?;

    let res = wrap_near(
        contract.as_account(),
        &contract,
        &worker,
        parse_near!("3 N"),
    )
    .await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    // call fails because defi contract is not registered as a wNEAR account
    let res = contract
        .call(&worker, "ft_transfer_call")
        .args_json((
            defi_contract.id(),
            parse_near!("1 N"),
            Option::<String>::None,
            "invest",
        ))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::Failure(_)));

    // balances remain unchanged
    let root_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    let defi_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((defi_contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(root_balance.0, parse_near!("3 N"));
    assert_eq!(defi_balance.0, 0);

    Ok(())
}

#[tokio::test]
async fn test_transfer_call_with_promise_and_refund() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;
    let defi_contract = init_defi(&worker, &contract).await?;

    register_user(&contract.as_account(), &contract, &worker).await?;
    register_user(&defi_contract.as_account(), &contract, &worker).await?;

    let res = wrap_near(
        contract.as_account(),
        &contract,
        &worker,
        parse_near!("3 N"),
    )
    .await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let res = contract
        .call(&worker, "ft_transfer_call")
        .args_json((
            defi_contract.id(),
            U128::from(parse_near!("1 N")),
            Option::<String>::None,
            U128::from(parse_near!("0.5 N")),
        ))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    let root_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    let defi_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((defi_contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(root_balance.0, parse_near!("2.5 N"));
    assert_eq!(defi_balance.0, parse_near!("0.5 N"));

    Ok(())
}

#[tokio::test]
async fn test_transfer_call_promise_panics_for_a_full_refund() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let contract = init_w_near(&worker).await?;
    let defi_contract = init_defi(&worker, &contract).await?;

    register_user(&contract.as_account(), &contract, &worker).await?;
    register_user(&defi_contract.as_account(), &contract, &worker).await?;

    let res = wrap_near(
        contract.as_account(),
        &contract,
        &worker,
        parse_near!("3 N"),
    )
    .await?;
    assert!(matches!(res, FinalExecutionStatus::SuccessValue(_)));

    let res = contract
        .call(&worker, "ft_transfer_call")
        .args_json((
            defi_contract.id(),
            U128::from(parse_near!("1 N")),
            Option::<String>::None,
            "invalid integer".to_string(),
        ))?
        .gas(300_000_000_000_000)
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    // balances remain unchanged
    let root_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    let defi_balance = contract
        .call(&worker, "ft_balance_of")
        .args_json((defi_contract.id(),))?
        .view()
        .await?
        .json::<U128>()?;
    assert_eq!(root_balance.0, parse_near!("3 N"));
    assert_eq!(defi_balance.0, 0);

    Ok(())
}
