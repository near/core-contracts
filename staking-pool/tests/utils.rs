extern crate staking_pool;

use near_crypto::{InMemorySigner, KeyType, Signer};
use near_primitives::{
    account::AccessKey,
    hash::CryptoHash,
    transaction::{ExecutionOutcome, ExecutionStatus, Transaction},
    types::{AccountId, Balance},
};
use near_runtime_standalone::{init_runtime_and_signer, RuntimeStandalone};
use near_sdk::json_types::U128;
use serde_json::json;
use staking_pool::RewardFeeFraction;

pub const POOL_ACCOUNT_ID: &str = "pool";

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

lazy_static::lazy_static! {
    static ref POOL_WASM_BYTES: &'static [u8] = include_bytes!("../res/staking_pool.wasm").as_ref();
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

    pub fn create_external(
        &self,
        runtime: &mut RuntimeStandalone,
        new_account_id: AccountId,
        amount: Balance,
    ) -> ExternalUser {
        let new_signer =
            InMemorySigner::from_seed(&new_account_id, KeyType::ED25519, &new_account_id);
        let tx = self
            .new_tx(runtime, new_account_id.clone())
            .create_account()
            .add_key(new_signer.public_key(), AccessKey::full_access())
            .transfer(amount)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        ExternalUser {
            account_id: new_account_id,
            signer: new_signer,
        }
    }

    pub fn pool_init_new(
        &self,
        runtime: &mut RuntimeStandalone,
        amount: Balance,
        reward_fee_fraction: RewardFeeFraction,
    ) -> ExecutionOutcome {
        let args = json!({
            "owner_id": self.account_id,
            "stake_public_key": "ed25519:3tysLvy7KGoE8pznUgXvSHa4vYyGvrDZFcT8jgb8PEQ6", // not relevant for now
            "reward_fee_fraction": reward_fee_fraction
        })
        .to_string()
        .as_bytes()
        .to_vec();

        let tx = self
            .new_tx(runtime, POOL_ACCOUNT_ID.into())
            .create_account()
            .transfer(amount)
            .deploy_contract(POOL_WASM_BYTES.to_vec())
            .function_call("new".into(), args, 1000000000000, 0)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        res
    }

    pub fn pool_deposit(
        &self,
        runtime: &mut RuntimeStandalone,
        amount: Balance,
    ) -> ExecutionOutcome {
        let tx = self
            .new_tx(runtime, POOL_ACCOUNT_ID.into())
            .function_call("deposit".into(), vec![], 10000000000000000, amount)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        res
    }

    pub fn pool_ping(&self, runtime: &mut RuntimeStandalone) -> ExecutionOutcome {
        let tx = self
            .new_tx(runtime, POOL_ACCOUNT_ID.into())
            .function_call("ping".into(), vec![], 10000000000000000, 0)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        res
    }

    pub fn pool_stake(&self, runtime: &mut RuntimeStandalone, amount: u128) -> ExecutionOutcome {
        let args = json!({ "amount": format!("{}", amount) })
            .to_string()
            .as_bytes()
            .to_vec();
        let tx = self
            .new_tx(runtime, POOL_ACCOUNT_ID.into())
            .function_call("stake".into(), args, 10000000000000000, 0)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        res
    }

    #[allow(dead_code)]
    pub fn pool_unstake(&self, runtime: &mut RuntimeStandalone, amount: u128) -> ExecutionOutcome {
        let args = json!({ "amount": format!("{}", amount) })
            .to_string()
            .as_bytes()
            .to_vec();
        let tx = self
            .new_tx(runtime, POOL_ACCOUNT_ID.into())
            .function_call("unstake".into(), args, 10000000000000000, 0)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        res
    }

    pub fn pool_withdraw(&self, runtime: &mut RuntimeStandalone, amount: u128) -> ExecutionOutcome {
        let args = json!({ "amount": format!("{}", amount) })
            .to_string()
            .as_bytes()
            .to_vec();
        let tx = self
            .new_tx(runtime, POOL_ACCOUNT_ID.into())
            .function_call("withdraw".into(), args, 10000000000000000, 0)
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        // assert!(matches!(res, ExecutionOutcome { status: ExecutionStatus::SuccessValue(_), ..}));
        runtime.process_all().unwrap();
        res
    }

    #[allow(dead_code)]
    pub fn get_account_staked_balance(&self, runtime: &RuntimeStandalone) -> Balance {
        let balance = runtime
            .view_method_call(
                &POOL_ACCOUNT_ID.into(),
                "get_account_staked_balance",
                json!({"account_id": self.account_id})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
            .0;
        u128::from(serde_json::from_slice::<U128>(balance.as_slice()).unwrap())
    }

    pub fn get_account_unstaked_balance(&self, runtime: &RuntimeStandalone) -> Balance {
        let balance = runtime
            .view_method_call(
                &POOL_ACCOUNT_ID.into(),
                "get_account_unstaked_balance",
                json!({"account_id": self.account_id})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap()
            .0;
        u128::from(serde_json::from_slice::<U128>(balance.as_slice()).unwrap())
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

pub fn init_pool(initial_transfer: Balance) -> (RuntimeStandalone, ExternalUser) {
    let (mut runtime, signer) = init_runtime_and_signer(&"root".into());
    let root = ExternalUser::new("root".into(), signer);

    root.pool_init_new(
        &mut runtime,
        initial_transfer,
        RewardFeeFraction {
            numerator: 10,
            denominator: 100,
        },
    );
    return (runtime, root);
}
