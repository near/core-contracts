use near_primitives::views::FinalExecutionStatus;
use near_sdk::json_types::U128;
use near_sdk::{Balance, ONE_YOCTO};
use near_units::parse_near;
use workspaces::prelude::DevAccountDeployer;
use workspaces::{Account, Contract, DevNetwork, Worker};

const LEGACY_BYTE_COST: Balance = 10_000_000_000_000_000_000;

const STORAGE_BALANCE: Balance = 125 * LEGACY_BYTE_COST;

pub async fn init_legacy(worker: &Worker<impl DevNetwork>) -> anyhow::Result<Contract> {
    let contract = worker
        .dev_deploy(include_bytes!("../res/legacy_w_near.wasm").to_vec())
        .await?;
    let res = contract
        .call(&worker, "new")
        .args_json((contract.id(),))?
        .gas(300_000_000_000_000)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    Ok(contract)
}

pub async fn init_w_near(worker: &Worker<impl DevNetwork>) -> anyhow::Result<Contract> {
    let contract = worker
        .dev_deploy(include_bytes!("../res/w_near.wasm").to_vec())
        .await?;
    let res = contract
        .call(&worker, "new")
        .args_json((contract.id(),))?
        .gas(300_000_000_000_000)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    Ok(contract)
}

async fn legacy_register_user(
    name: &str,
    contract: &Contract,
    worker: &Worker<impl DevNetwork>,
) -> anyhow::Result<Account> {
    let res = contract
        .as_account()
        .create_subaccount(&worker, name)
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?;
    assert!(matches!(
        res.details.status,
        FinalExecutionStatus::SuccessValue(_)
    ));
    let account = res.result;

    let res = contract
        .call(&worker, "storage_deposit")
        .args_json((account.id(),))?
        .gas(300_000_000_000_000)
        .deposit(125 * LEGACY_BYTE_COST)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    Ok(account)
}

async fn register_user(
    account: &Account,
    contract: &Contract,
    worker: &Worker<impl DevNetwork>,
) -> anyhow::Result<()> {
    let res = contract
        .call(&worker, "storage_deposit")
        .args_json((account.id(), Option::<bool>::None))?
        .gas(300_000_000_000_000)
        .deposit(125 * LEGACY_BYTE_COST)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    Ok(())
}

async fn create_user(
    name: &str,
    contract: &Contract,
    worker: &Worker<impl DevNetwork>,
) -> anyhow::Result<Account> {
    let res = contract
        .as_account()
        .create_subaccount(&worker, name)
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?;
    assert!(matches!(
        res.details.status,
        FinalExecutionStatus::SuccessValue(_)
    ));
    let account = res.result;

    register_user(&account, &contract, &worker).await?;

    Ok(account)
}

async fn wrap_near(
    account: &Account,
    contract: &Contract,
    worker: &Worker<impl DevNetwork>,
    amount: Balance,
) -> anyhow::Result<FinalExecutionStatus> {
    Ok(account
        .call(&worker, contract.id().clone(), "near_deposit")
        .args_json((account.id(),))?
        .gas(300_000_000_000_000)
        .deposit(amount)
        .transact()
        .await?
        .status)
}

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
        .deploy(&worker, include_bytes!("../res/w_near.wasm").to_vec())
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
