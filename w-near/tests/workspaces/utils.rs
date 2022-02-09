use near_primitives::views::FinalExecutionStatus;
use near_sdk::Balance;
use near_units::parse_near;
use workspaces::prelude::DevAccountDeployer;
use workspaces::{Account, Contract, DevNetwork, Worker};

pub const LEGACY_BYTE_COST: Balance = 10_000_000_000_000_000_000;

pub const STORAGE_BALANCE: Balance = 125 * LEGACY_BYTE_COST;

pub async fn init_legacy(worker: &Worker<impl DevNetwork>) -> anyhow::Result<Contract> {
    let contract = worker
        .dev_deploy(include_bytes!("../../res/legacy_w_near.wasm").to_vec())
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
        .dev_deploy(include_bytes!("../../res/w_near.wasm").to_vec())
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
        .dev_deploy(include_bytes!("../../res/w_near_defi.wasm").to_vec())
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

pub async fn legacy_register_user(
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

pub async fn register_user(
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

pub async fn create_user(
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

pub async fn wrap_near(
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
