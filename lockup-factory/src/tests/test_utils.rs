use near_sdk::json_types::ValidAccountId;
use near_sdk::{env, Balance, BlockHeight, EpochHeight};
use near_sdk::{AccountId, MockedBlockchain, PromiseResult, VMContext};
use std::convert::TryInto;

pub const GENESIS_TIME_IN_DAYS: u64 = 500;
pub const YEAR: u64 = 365;
pub const SALT: [u8; 3] = [1, 2, 3];

pub fn to_ts(num_days: u64) -> u64 {
    // 2018-08-01 UTC in nanoseconds
    1533081600_000_000_000 + to_nanos(num_days)
}

pub fn to_nanos(num_days: u64) -> u64 {
    num_days * 86400_000_000_000
}

pub fn account_near() -> AccountId {
    "nearnet".to_string()
}

pub fn account_factory() -> AccountId {
    "lockup.nearnet".to_string()
}

pub fn lockup_master_account_id() -> ValidAccountId {
    "lockup.nearnet".try_into().unwrap()
}

pub fn whitelist_account_id() -> ValidAccountId {
    "whitelist.nearnet".try_into().unwrap()
}

pub fn foundation_account_id() -> ValidAccountId {
    "nearnet".try_into().unwrap()
}

pub fn account_tokens_owner() -> ValidAccountId {
    "tokenowner.testnet".try_into().unwrap()
}

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

pub fn lockup_account() -> AccountId {
    let byte_slice = env::sha256(account_tokens_owner().as_ref().as_bytes());
    let lockup_account_id = format!(
        "{}.{}",
        hex::encode(&byte_slice[..20]),
        &lockup_master_account_id().as_ref().to_string()
    );
    return lockup_account_id;
}

pub fn testing_env_with_promise_results(context: VMContext, promise_result: PromiseResult) {
    let storage = near_sdk::env::take_blockchain_interface()
        .unwrap()
        .as_mut_mocked_blockchain()
        .unwrap()
        .take_storage();

    near_sdk::env::set_blockchain_interface(Box::new(MockedBlockchain::new(
        context,
        Default::default(),
        Default::default(),
        vec![promise_result],
        storage,
        Default::default(),
        None
    )));
}

pub struct VMContextBuilder {
    context: VMContext,
}

impl VMContextBuilder {
    pub fn new() -> Self {
        Self {
            context: VMContext {
                current_account_id: "".to_string(),
                signer_account_id: "".to_string(),
                signer_account_pk: vec![0, 1, 2],
                predecessor_account_id: "".to_string(),
                input: vec![],
                epoch_height: 0,
                block_index: 0,
                block_timestamp: 0,
                account_balance: 0,
                account_locked_balance: 0,
                storage_usage: 10u64.pow(6),
                attached_deposit: 0,
                prepaid_gas: 10u64.pow(18),
                random_seed: vec![0, 1, 2],
                is_view: false,
                output_data_receivers: vec![],
            },
        }
    }

    #[allow(dead_code)]
    pub fn current_account_id(mut self, account_id: AccountId) -> Self {
        self.context.current_account_id = account_id;
        self
    }

    #[allow(dead_code)]
    pub fn signer_account_id(mut self, account_id: AccountId) -> Self {
        self.context.signer_account_id = account_id;
        self
    }

    #[allow(dead_code)]
    pub fn predecessor_account_id(mut self, account_id: AccountId) -> Self {
        self.context.predecessor_account_id = account_id;
        self
    }

    #[allow(dead_code)]
    pub fn block_index(mut self, block_index: BlockHeight) -> Self {
        self.context.block_index = block_index;
        self
    }

    #[allow(dead_code)]
    pub fn epoch_height(mut self, epoch_height: EpochHeight) -> Self {
        self.context.epoch_height = epoch_height;
        self
    }

    #[allow(dead_code)]
    pub fn attached_deposit(mut self, amount: Balance) -> Self {
        self.context.attached_deposit = amount;
        self
    }

    #[allow(dead_code)]
    pub fn account_balance(mut self, amount: Balance) -> Self {
        self.context.account_balance = amount;
        self
    }

    #[allow(dead_code)]
    pub fn account_locked_balance(mut self, amount: Balance) -> Self {
        self.context.account_locked_balance = amount;
        self
    }

    pub fn finish(self) -> VMContext {
        self.context
    }
}
