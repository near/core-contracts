use std::collections::HashSet;

use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::{env, near_bindgen, AccountId, Balance, Gas, Promise, PublicKey};

const DEFAULT_ALLOWANCE: u128 = 1_000_000_000_000_000_000_000;

pub type RequestId = u32;

#[derive(BorshDeserialize, BorshSerialize)]
pub enum MultiSigRequest {
    Transfer {
        account_id: AccountId,
        amount: Balance,
    },
    AddKey {
        public_key: PublicKey,
    },
    DeleteKey {
        public_key: PublicKey,
    },
    FunctionCall {
        contract_id: AccountId,
        method_name: String,
        args: Vec<u8>,
        deposit: Balance,
        gas: Gas,
    },
    SetNumConfirmations {
        num_confirmations: u32,
    },
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct MultiSigContract {
    num_confirmations: u32,
    request_nonce: RequestId,
    requests: Map<RequestId, MultiSigRequest>,
    confirmations: Map<RequestId, HashSet<PublicKey>>,
}

impl Default for MultiSigContract {
    fn default() -> Self {
        env::panic(b"Multisig contract should be initialized before usage")
    }
}

#[near_bindgen]
impl MultiSigContract {
    /// Initialize multisig contract.
    /// @params num_confirmations: k of n signatures required to perform operations.
    #[init]
    pub fn new(num_confirmations: u32) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            num_confirmations,
            request_nonce: 0,
            requests: Default::default(),
            confirmations: Default::default(),
        }
    }

    fn add_request(&mut self, request: MultiSigRequest) -> RequestId {
        if self.num_confirmations == 1 {
            self.execute_request(request);
            0
        } else {
            self.requests.insert(&self.request_nonce, &request);
            let confirmations = vec![env::signer_account_pk()].into_iter().collect();
            self.confirmations
                .insert(&self.request_nonce, &confirmations);
            self.request_nonce += 1;
            self.request_nonce - 1
        }
    }

    fn execute_request(&mut self, request: MultiSigRequest) {
        match request {
            MultiSigRequest::Transfer { account_id, amount } => {
                Promise::new(account_id).transfer(amount);
            }
            MultiSigRequest::AddKey { public_key } => {
                Promise::new(env::current_account_id()).add_access_key(
                    public_key,
                    DEFAULT_ALLOWANCE,
                    env::current_account_id(),
                    "transfer,function_call,add_key,confirm"
                        .to_string()
                        .into_bytes(),
                );
            }
            MultiSigRequest::DeleteKey { public_key } => {
                Promise::new(env::current_account_id()).delete_key(public_key);
            }
            MultiSigRequest::FunctionCall {
                contract_id,
                method_name,
                args,
                deposit,
                gas,
            } => {
                Promise::new(contract_id).function_call(
                    method_name.into_bytes(),
                    args,
                    deposit,
                    gas,
                );
            }
            MultiSigRequest::SetNumConfirmations { num_confirmations } => {
                self.num_confirmations = num_confirmations;
            }
        };
    }

    /// Request to change number of confirmations.
    pub fn set_num_confirmations(&mut self, num_confirmations: u32) -> RequestId {
        assert_eq!(
            env::current_account_id(),
            env::signer_account_id(),
            "Signer account must much given account"
        );
        self.add_request(MultiSigRequest::SetNumConfirmations { num_confirmations })
    }

    /// Request to transfer funds from this account to given account.
    pub fn transfer(&mut self, account_id: AccountId, amount: U128) -> RequestId {
        // TOOD: add safety margin for storage allocated balance.
        assert_eq!(
            env::current_account_id(),
            env::signer_account_id(),
            "Signer account must much given account"
        );
        assert!(
            env::account_balance() >= amount.0,
            "Not enough funds to initiate transfer"
        );
        self.add_request(MultiSigRequest::Transfer {
            account_id,
            amount: amount.0,
        })
    }

    /// Request to add new access key to this contract.
    /// Allows to extend allowed set of keys.
    pub fn add_key(&mut self, public_key: Base58PublicKey) -> RequestId {
        assert_eq!(
            env::current_account_id(),
            env::signer_account_id(),
            "Signer account must much given account"
        );
        self.add_request(MultiSigRequest::AddKey {
            public_key: public_key.into(),
        })
    }

    /// Request to delete existing access key from this contract.
    /// Allows to remove not used / lost / exposed key.
    pub fn delete_key(&mut self, public_key: Base58PublicKey) -> RequestId {
        assert_eq!(
            env::current_account_id(),
            env::signer_account_id(),
            "Signer account must much given account"
        );
        self.add_request(MultiSigRequest::DeleteKey {
            public_key: public_key.into(),
        })
    }

    /// Request to call given contract's method with given argument / deposit / gas.
    pub fn function_call(
        &mut self,
        contract_id: AccountId,
        method_name: String,
        args: Vec<u8>,
        deposit: Balance,
        gas: Gas,
    ) -> RequestId {
        assert_eq!(
            env::current_account_id(),
            env::signer_account_id(),
            "Signer account must much given account"
        );
        self.add_request(MultiSigRequest::FunctionCall {
            contract_id,
            method_name,
            args,
            deposit,
            gas,
        })
    }

    /// Confirm given request with given signing key.
    /// If with this, there has been enough confirmation, a promise with request will be scheduled.
    pub fn confirm(&mut self, request_id: RequestId) {
        assert_eq!(
            env::current_account_id(),
            env::signer_account_id(),
            "Signer account must much given account"
        );
        assert!(
            self.requests.get(&request_id).is_some(),
            "No such request: either wrong number or already confirmed"
        );
        assert!(
            self.confirmations.get(&request_id).is_some(),
            "Internal error: confirmations mismatch requests"
        );
        let mut confirmations = self.confirmations.get(&request_id).unwrap();
        assert!(
            !confirmations.contains(&env::signer_account_pk()),
            "Already confirmed this request"
        );
        if confirmations.len() as u32 + 1 >= self.num_confirmations {
            let request = self
                .requests
                .remove(&request_id)
                .expect("Failed to remove existing element");
            self.execute_request(request);
            self.confirmations.remove(&request_id);
        } else {
            confirmations.insert(env::signer_account_pk());
            self.confirmations.insert(&request_id, &confirmations);
        }
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{testing_env, MockedBlockchain};
    use near_sdk::{AccountId, VMContext};
    use near_sdk::{Balance, BlockHeight, EpochHeight};

    use super::*;

    pub fn alice() -> AccountId {
        "alice".to_string()
    }
    pub fn bob() -> AccountId {
        "bob".to_string()
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

        pub fn current_account_id(mut self, account_id: AccountId) -> Self {
            self.context.current_account_id = account_id;
            self
        }

        #[allow(dead_code)]
        pub fn signer_account_id(mut self, account_id: AccountId) -> Self {
            self.context.signer_account_id = account_id;
            self
        }

        pub fn signer_account_pk(mut self, signer_account_pk: PublicKey) -> Self {
            self.context.signer_account_pk = signer_account_pk;
            self
        }

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

    fn context_with_key(key: PublicKey, amount: Balance) -> VMContext {
        VMContextBuilder::new()
            .current_account_id(alice())
            .predecessor_account_id(alice())
            .signer_account_id(alice())
            .signer_account_pk(key)
            .account_balance(amount)
            .finish()
    }

    #[test]
    fn test_multi_3_of_n() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![1, 2, 3], amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.transfer(bob(), amount.into());
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 1);
        testing_env!(context_with_key(vec![3, 2, 1], amount));
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 2);
        testing_env!(context_with_key(vec![5, 7, 9], amount));
        c.confirm(request_id);
        // TODO: confirm that funds were transferred out via promise.
        assert_eq!(c.requests.len(), 0);
    }

    #[test]
    fn test_change_num_confirmations() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![1, 2, 3], amount));
        let mut c = MultiSigContract::new(1);
        c.set_num_confirmations(2);
        assert_eq!(c.num_confirmations, 2);
    }

    #[test]
    #[should_panic]
    fn test_panics_on_second_confirm() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![5, 7, 9], amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.transfer(bob(), amount.into());
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 1);
        c.confirm(request_id);
    }
}
