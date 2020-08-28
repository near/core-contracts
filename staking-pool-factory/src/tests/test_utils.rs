use near_sdk::{AccountId, MockedBlockchain, PromiseResult, VMContext};
use near_sdk::{Balance, BlockHeight, EpochHeight};

pub fn account_near() -> AccountId {
    "near".to_string()
}
pub fn account_whitelist() -> AccountId {
    "whitelist".to_string()
}
pub fn staking_pool_id() -> AccountId {
    "pool".to_string()
}
pub fn account_pool() -> AccountId {
    "pool.factory".to_string()
}
pub fn account_factory() -> AccountId {
    "factory".to_string()
}
pub fn account_tokens_owner() -> AccountId {
    "tokens-owner".to_string()
}
pub fn account_pool_owner() -> AccountId {
    "pool-owner".to_string()
}

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
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
