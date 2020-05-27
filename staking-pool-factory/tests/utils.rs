#![allow(dead_code)]

use near_crypto::{InMemorySigner, KeyType, Signer};
use near_primitives::{
    account::{AccessKey, Account},
    errors::{RuntimeError, TxExecutionError},
    hash::CryptoHash,
    transaction::{ExecutionOutcome, ExecutionStatus, Transaction},
    types::{AccountId, Balance},
};
use near_runtime_standalone::{init_runtime_and_signer, RuntimeStandalone};
use serde::de::DeserializeOwned;
use serde_json::json;

pub const FACTORY_ACCOUNT_ID: &str = "factory";

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

lazy_static::lazy_static! {
    static ref FACTORY_WASM_BYTES: &'static [u8] = include_bytes!("../res/staking_pool_factory.wasm").as_ref();
    static ref WHITELIST_WASM_BYTES: &'static [u8] = include_bytes!("../../whitelist/res/whitelist.wasm").as_ref();
}

type TxResult = Result<ExecutionOutcome, ExecutionOutcome>;

fn outcome_into_result(outcome: ExecutionOutcome) -> TxResult {
    match outcome.status {
        ExecutionStatus::SuccessValue(_) => Ok(outcome),
        ExecutionStatus::Failure(_) => Err(outcome),
        ExecutionStatus::SuccessReceiptId(_) => panic!("Unresolved ExecutionOutcome run runitme.resolve(tx) to resolve the filnal outcome of tx"),
        ExecutionStatus::Unknown => unreachable!()
    }
}

pub struct ExternalUser {
    pub account_id: AccountId,
    pub signer: InMemorySigner,
}

impl ExternalUser {
    pub fn new(account_id: AccountId, signer: InMemorySigner) -> Self {
        Self { account_id, signer }
    }

    #[allow(dead_code)]
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    #[allow(dead_code)]
    pub fn signer(&self) -> &InMemorySigner {
        &self.signer
    }

    pub fn account(&self, runtime: &RuntimeStandalone) -> Account {
        runtime
            .view_account(&self.account_id)
            .expect("Account should be there")
    }

    pub fn create_external(
        &self,
        runtime: &mut RuntimeStandalone,
        new_account_id: AccountId,
        amount: Balance,
    ) -> Result<ExternalUser, ExecutionOutcome> {
        let new_signer =
            InMemorySigner::from_seed(&new_account_id, KeyType::ED25519, &new_account_id);
        let tx = self
            .new_tx(runtime, new_account_id.clone())
            .create_account()
            .add_key(new_signer.public_key(), AccessKey::full_access())
            .transfer(amount)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx);

        // TODO: this temporary hack, must be rewritten
        if let Err(err) = res.clone() {
            if let RuntimeError::InvalidTxError(tx_err) = err {
                let mut out = ExecutionOutcome::default();
                out.status = ExecutionStatus::Failure(TxExecutionError::InvalidTxError(tx_err));
                return Err(out);
            } else {
                unreachable!();
            }
        } else {
            outcome_into_result(res.unwrap())?;
            runtime.process_all().unwrap();
            Ok(ExternalUser {
                account_id: new_account_id,
                signer: new_signer,
            })
        }
    }

    pub fn transfer(
        &self,
        runtime: &mut RuntimeStandalone,
        receiver_id: &str,
        amount: Balance,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, receiver_id.to_string())
            .transfer(amount)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        runtime.process_all().unwrap();
        outcome_into_result(res)
    }

    pub fn function_call(
        &self,
        runtime: &mut RuntimeStandalone,
        receiver_id: &str,
        method: &str,
        args: &[u8],
        deposit: u128,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, receiver_id.to_string())
            .function_call(method.into(), args.to_vec(), 10000000000000000, deposit)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        runtime.process_all().unwrap();
        outcome_into_result(res)
    }

    pub fn init_factory(
        &self,
        runtime: &mut RuntimeStandalone,
        staking_pool_whitelist_account_id: &str,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, FACTORY_ACCOUNT_ID.into())
            .create_account()
            .transfer(ntoy(60))
            .deploy_contract(FACTORY_WASM_BYTES.to_vec())
            .function_call(
                "new".into(),
                serde_json::to_vec(&json!({"staking_pool_whitelist_account_id": staking_pool_whitelist_account_id.to_string()})).unwrap(),
                10000000000000000,
                0,
            )
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        runtime.process_all().unwrap();
        outcome_into_result(res)
    }

    pub fn init_whitelist(
        &self,
        runtime: &mut RuntimeStandalone,
        staking_pool_whitelist_account_id: AccountId,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, staking_pool_whitelist_account_id)
            .create_account()
            .transfer(ntoy(30))
            .deploy_contract(WHITELIST_WASM_BYTES.to_vec())
            .function_call(
                "new".into(),
                serde_json::to_vec(&json!({"foundation_account_id": self.account_id()})).unwrap(),
                1000000000000000,
                0,
            )
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        runtime.process_all().unwrap();
        outcome_into_result(res)
    }

    fn new_tx(&self, runtime: &RuntimeStandalone, receiver_id: AccountId) -> Transaction {
        let nonce = runtime
            .view_access_key(&self.account_id, &self.signer.public_key())
            .unwrap()
            .nonce
            + 1;
        Transaction::new(
            self.account_id.clone(),
            self.signer.public_key(),
            receiver_id,
            nonce,
            CryptoHash::default(),
        )
    }
}

pub fn wait_epoch(runtime: &mut RuntimeStandalone) {
    let epoch_height = runtime.current_block().epoch_height;
    while epoch_height == runtime.current_block().epoch_height {
        runtime.produce_block().unwrap();
    }
}

pub fn view_factory<I: ToString, O: DeserializeOwned>(
    runtime: &RuntimeStandalone,
    method: &str,
    args: I,
) -> O {
    call_view(runtime, &FACTORY_ACCOUNT_ID, method, args)
}

pub fn call_view<I: ToString, O: DeserializeOwned>(
    runtime: &RuntimeStandalone,
    account_id: &str,
    method: &str,
    args: I,
) -> O {
    let args = args.to_string();
    let result = runtime
        .view_method_call(&account_id.to_string(), method, args.as_bytes())
        .unwrap()
        .0;
    let output: O = serde_json::from_reader(result.as_slice()).unwrap();
    output
}

pub fn new_root(account_id: AccountId) -> (RuntimeStandalone, ExternalUser) {
    let (runtime, signer) = init_runtime_and_signer(&account_id);
    (runtime, ExternalUser { account_id, signer })
}
