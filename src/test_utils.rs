use near_bindgen::{Balance, BlockHeight};
use near_bindgen::{AccountId, VMContext};

pub fn staking() -> AccountId {
    "staking".to_string()
}

pub fn bob() -> AccountId {
    "bob".to_string()
}

pub struct VMContextBuilder {
    context: VMContext
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
                epoch_id: [0u8; 32],
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
            }
        }
    }

    pub fn current_account_id(mut self, account_id: AccountId) -> Self {
        self.context.current_account_id = account_id;
        self
    }

    pub fn signer_account_id(mut self, account_id: AccountId) -> Self {
        self.context.signer_account_id = account_id;
        self
    }

    pub fn predecessor_account_id(mut self, account_id: AccountId) -> Self {
        self.context.predecessor_account_id = account_id;
        self
    }

    pub fn block_index(mut self, block_index: BlockHeight) -> Self {
        self.context.block_index = block_index;
        self
    }

    pub fn attached_deposit(mut self, amount: Balance) -> Self {
        self.context.attached_deposit = amount;
        self
    }

    pub fn account_balance(mut self, amount: Balance) -> Self {
        self.context.account_balance = amount;
        self
    }

    pub fn account_locked_balance(mut self, amount: Balance) -> Self {
        self.context.account_locked_balance = amount;
        self
    }

    pub fn finish(self) -> VMContext {
        self.context
    }
}
