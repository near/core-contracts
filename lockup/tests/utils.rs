#![allow(dead_code)]

extern crate lockup_contract;

use lockup_contract::types::LockupStartInformation;
use near_crypto::{InMemorySigner, KeyType, Signer};
use near_primitives::{
    account::{AccessKey, Account},
    errors::{RuntimeError, TxExecutionError},
    hash::CryptoHash,
    transaction::{ExecutionOutcome, ExecutionStatus, Transaction},
    types::{AccountId, Balance},
};
use near_runtime_standalone::{init_runtime_and_signer, RuntimeStandalone};
use near_sdk::json_types::{Base58PublicKey, U128, U64};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;

pub const LOCKUP_ACCOUNT_ID: &str = "holder";

pub fn days(num_days: u64) -> u64 {
    num_days * 86400_000_000_000
}

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

pub fn public_key(byte_val: u8) -> Base58PublicKey {
    let mut pk = vec![byte_val; 33];
    pk[0] = 0;
    Base58PublicKey(pk)
}

lazy_static::lazy_static! {
    static ref POOL_WASM_BYTES: &'static [u8] = include_bytes!("../res/lockup_contract.wasm").as_ref();
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

#[derive(Serialize)]
pub struct InitLockupArgs {
    pub lockup_duration: U64,
    pub lockup_start_information: LockupStartInformation,
    pub staking_pool_whitelist_account_id: AccountId,
    pub transfer_poll_account_id: Option<AccountId>,
    pub initial_owners_main_public_key: Base58PublicKey,
    pub owners_staking_public_key: Option<Base58PublicKey>,
    pub foundation_account_id: Option<AccountId>,
}

pub struct ExternalUser {
    account_id: AccountId,
    signer: InMemorySigner,
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

    pub fn account(&self, runtime: &mut RuntimeStandalone) -> Account {
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
        amount: Balance,
        receiver_id: AccountId,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, LOCKUP_ACCOUNT_ID.into())
            .function_call(
                "transfer".into(),
                serde_json::to_vec(&json!({"amount": U128(amount), "receiver_id": receiver_id}))
                    .unwrap(),
                10000000000000000,
                0,
            )
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        runtime.process_all().unwrap();
        outcome_into_result(res)
    }

    pub fn init_lockup(
        &self,
        runtime: &mut RuntimeStandalone,
        args: &InitLockupArgs,
        amount: Balance,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, LOCKUP_ACCOUNT_ID.into())
            .create_account()
            .transfer(ntoy(30) + amount)
            .deploy_contract(POOL_WASM_BYTES.to_vec())
            .function_call(
                "new".into(),
                serde_json::to_vec(args).unwrap(),
                10000000000000000,
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

pub fn call_lockup<I: ToString, O: DeserializeOwned>(
    runtime: &mut RuntimeStandalone,
    method: &str,
    args: I,
) -> O {
    call_view(runtime, &LOCKUP_ACCOUNT_ID.into(), method, args)
}

pub fn lockup_account(runtime: &mut RuntimeStandalone) -> Account {
    runtime.view_account(&LOCKUP_ACCOUNT_ID.into()).unwrap()
}

pub fn call_view<I: ToString, O: DeserializeOwned>(
    runtime: &mut RuntimeStandalone,
    account_id: &AccountId,
    method: &str,
    args: I,
) -> O {
    let args = args.to_string();
    let result = runtime
        .view_method_call(account_id, method, args.as_bytes())
        .unwrap()
        .0;
    let output: O = serde_json::from_reader(result.as_slice()).unwrap();
    output
}

pub fn new_root(account_id: AccountId) -> (RuntimeStandalone, ExternalUser) {
    let (runtime, signer) = init_runtime_and_signer(&account_id);
    (runtime, ExternalUser { account_id, signer })
}
