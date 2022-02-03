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

pub async fn init_defi(
    worker: &Worker<impl DevNetwork>,
    contract: &Contract,
) -> anyhow::Result<Contract> {
    let defi_contract = worker
        .dev_deploy(include_bytes!("../res/w_near_defi.wasm").to_vec())
        .await?;
    let res = defi_contract
        .call(&worker, "new")
        .args_json((contract.id(),))?
        .gas(300_000_000_000_000)
        .transact()
        .await?;
    assert!(matches!(res.status, FinalExecutionStatus::SuccessValue(_)));

    Ok(defi_contract)
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
