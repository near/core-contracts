use std::collections::HashSet;
use std::convert::TryFrom;

use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::json_types::{Base58PublicKey, Base64VecU8, U128, U64};
use near_sdk::{env, near_bindgen, AccountId, Promise, PromiseOrValue, PublicKey};
use serde::{Deserialize, Serialize};

/// Unlimited allowance for multisig keys.
const DEFAULT_ALLOWANCE: u128 = 0;
const NANO_TO_SECONDS: u64 = 1000000000;

pub type RequestId = u32;

/// Permissions for function call access key.
#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct FunctionCallPermission {
    allowance: Option<U128>,
    receiver_id: AccountId,
    method_names: Vec<String>,
}

#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MultiSigRequestAction {
    /// Transfers given amount to receiver.
    Transfer {
        amount: U128,
    },
    /// Create a new account.
    CreateAccount,
    /// Deploys contract to receiver's account. Can upgrade given contract as well.
    DeployContract { code: Base64VecU8 },
    /// Adds key, either new key for multisig or full access key to another account.
    AddKey {
        public_key: Base58PublicKey,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission: Option<FunctionCallPermission>,
    },
    /// Deletes key, either one of the keys from multisig or key from another account.
    DeleteKey {
        public_key: Base58PublicKey,
    },
    /// Call function on behalf of this contract.
    FunctionCall {
        method_name: String,
        args: Base64VecU8,
        deposit: U128,
        gas: U64,
    },
    /// Sets number of confirmations required to authorize requests.
    /// Can not be bundled with any other actions or transactions.
    SetNumConfirmations {
        num_confirmations: u32,
    },
    /// Sets expiry time of requests
    /// Can not be bundled with any other actions or transactions.
    SetExpiry {
        expiry: u32,
    },
    /// Sets the max lifespan of requests
    /// Can not be bundled with any other actions or transactions.
    SetLifespan {
        lifespan: u32,
    },
}

#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct MultiSigRequest {
    receiver_id: AccountId,
    actions: Vec<MultiSigRequestAction>,
}

#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct MultiSigRequestAdded {
    request: MultiSigRequest,
    added: u64,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct MultiSigContract {
    num_confirmations: u32,
    request_nonce: RequestId,
    requests: Map<RequestId, MultiSigRequestAdded>,
    confirmations: Map<RequestId, HashSet<PublicKey>>,
    // in seconds - the request is invalid after this time
    request_expiry: u32,
    // in seconds - the request cannot be deleted before this time
    request_lifespan: u32,
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
            request_expiry: 3600,
            request_lifespan: 180,
            requests: Map::new(b"r".to_vec()),
            confirmations: Map::new(b"c".to_vec()),
        }
    }

    /// Add request for multisig.
    pub fn add_request(&mut self, request: MultiSigRequest) -> RequestId {
        assert_eq!(
            env::current_account_id(),
            env::predecessor_account_id(),
            "Predecessor account must much current account"
        );
        let request_added = MultiSigRequestAdded {
            added: env::block_timestamp(),
            request: request,
        };
        self.requests.insert(&self.request_nonce, &request_added);
        let confirmations = HashSet::new();
        self.confirmations
            .insert(&self.request_nonce, &confirmations);
        self.request_nonce += 1;
        self.request_nonce - 1
    }

    /// Remove given request and associated confirmations.
    pub fn delete_request(&mut self, request_id: RequestId) -> bool {
        self.valid_request(request_id);
        let added = self.requests.get(&request_id).expect("No such request").added;
        if env::block_timestamp() - added > self.request_lifespan as u64 * NANO_TO_SECONDS {
            self.requests.remove(&request_id);
            self.confirmations.remove(&request_id);
            true
        } else {
            false
        }
    }

    fn execute_request(&mut self, request: MultiSigRequest) -> PromiseOrValue<bool> {
        let mut promise = Promise::new(request.receiver_id.clone());
        let num_actions = request.actions.len();
        for action in request.actions {
            promise = match action {
                MultiSigRequestAction::Transfer { amount } => promise.transfer(amount.into()),
                MultiSigRequestAction::CreateAccount => promise.create_account(),
                MultiSigRequestAction::DeployContract { code } => {
                    promise.deploy_contract(code.into())
                }
                MultiSigRequestAction::AddKey {
                    public_key,
                    permission,
                } if request.receiver_id == env::current_account_id() => {
                    assert!(
                        permission.is_none(),
                        "Permissions for access key on this contract are set by default"
                    );
                    promise
                        .add_access_key(
                            public_key.into(),
                            DEFAULT_ALLOWANCE,
                            env::current_account_id(),
                            "add_request,delete_request,confirm"
                                .to_string()
                                .into_bytes(),
                        )
                        .into()
                }
                MultiSigRequestAction::AddKey {
                    public_key,
                    permission,
                } => {
                    if let Some(permission) = permission {
                        promise.add_access_key(
                            public_key.into(),
                            permission
                                .allowance
                                .map(|x| x.into())
                                .unwrap_or(DEFAULT_ALLOWANCE),
                            permission.receiver_id,
                            permission
                                .method_names.join(",").into_bytes(),
                        )
                    } else {
                        promise.add_full_access_key(public_key.into())
                    }
                }
                MultiSigRequestAction::DeleteKey { public_key } => {
                    promise.delete_key(public_key.into())
                }
                MultiSigRequestAction::FunctionCall {
                    method_name,
                    args,
                    deposit,
                    gas,
                } => promise.function_call(
                    method_name.into_bytes(),
                    args.into(),
                    deposit.into(),
                    gas.into(),
                ),
                // the following must be a single action request with receiver_id == contract_id
                MultiSigRequestAction::SetNumConfirmations { num_confirmations } => {
                    assert_eq!(request.receiver_id, env::current_account_id(), "Changing number of confirmations only works when receiver_id is equal to current_account_id");
                    assert_eq!(
                        num_actions, 1,
                        "Changing number of confirmations should be a separate request"
                    );
                    self.num_confirmations = num_confirmations;
                    return PromiseOrValue::Value(true);
                }
                MultiSigRequestAction::SetExpiry { expiry } => {
                    assert_eq!(request.receiver_id, env::current_account_id(), "Changing expiry only works when receiver_id is equal to current_account_id");
                    assert_eq!(
                        num_actions, 1,
                        "Changing expiry should be a separate request"
                    );
                    self.request_expiry = expiry;
                    return PromiseOrValue::Value(true);
                }
                MultiSigRequestAction::SetLifespan { lifespan } => {
                    assert_eq!(request.receiver_id, env::current_account_id(), "Changing lifespan only works when receiver_id is equal to current_account_id");
                    assert_eq!(
                        num_actions, 1,
                        "Changing lifespan should be a separate request"
                    );
                    self.request_lifespan = lifespan;
                    return PromiseOrValue::Value(true);
                }
            };
        }
        promise.into()
    }

    /// Confirm given request with given signing key.
    /// If with this, there has been enough confirmation, a promise with request will be scheduled.
    pub fn confirm(&mut self, request_id: RequestId) -> PromiseOrValue<bool> {
        self.valid_request(request_id);
        let added = self.requests.get(&request_id).expect("No such request").added;
        if env::block_timestamp() - added > self.request_expiry as u64 * NANO_TO_SECONDS {
            self.requests.remove(&request_id);
            self.confirmations.remove(&request_id);
            PromiseOrValue::Value(false)
        } else {
            let mut confirmations = self.confirmations.get(&request_id).unwrap();
            assert!(
                !confirmations.contains(&env::signer_account_pk()),
                "Already confirmed this request with this key"
            );
            if confirmations.len() as u32 + 1 >= self.num_confirmations {
                let request = (self.requests.remove(&request_id).expect("Failed to remove existing element")).request;
                self.confirmations.remove(&request_id);
                self.execute_request(request)
            } else {
                confirmations.insert(env::signer_account_pk());
                self.confirmations.insert(&request_id, &confirmations);
                PromiseOrValue::Value(true)
            }
        }
    }

    pub fn get_request(&self, request_id: RequestId) -> MultiSigRequest {
        (self.requests.get(&request_id).expect("No such request")).request
    }

    pub fn list_request_ids(&self) -> Vec<RequestId> {
        self.requests.keys().collect()
    }

    pub fn get_confirmations(&self, request_id: RequestId) -> Vec<Base58PublicKey> {
        self.confirmations
            .get(&request_id)
            .expect("No such request")
            .into_iter()
            .map(|key| Base58PublicKey::try_from(key).expect("Failed to covert key to base58"))
            .collect()
    }

    pub fn get_num_confirmations(&self) -> u32 {
        self.num_confirmations
    }

    pub fn get_request_nonce(&self) -> u32 {
        self.request_nonce
    }
    
    // Helper checking that requested requests by request_id are valid - used by 3 functions
    fn valid_request(&mut self, request_id: RequestId) {
        assert_eq!(
            env::current_account_id(),
            env::predecessor_account_id(),
            "Predecessor account must much current account"
        );
        assert!(
            self.requests.get(&request_id).is_some(),
            "No such request: either wrong number or already confirmed"
        );
        assert!(
            self.confirmations.get(&request_id).is_some(),
            "Internal error: confirmations mismatch requests"
        );
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::{Debug, Error, Formatter};

    use near_sdk::{testing_env, MockedBlockchain};
    use near_sdk::{AccountId, VMContext};
    use near_sdk::{Balance, BlockHeight, EpochHeight};

    use super::*;

    /// Used for asserts_eq.
    /// TODO: replace with derive when https://github.com/near/near-sdk-rs/issues/165
    impl Debug for MultiSigRequest {
        fn fmt(&self, _f: &mut Formatter<'_>) -> Result<(), Error> {
            panic!("Should not trigger");
        }
    }

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

        pub fn block_timestamp(mut self, time: u64) -> Self {
            self.context.block_timestamp = time;
            self
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

    fn future_context_with_key(key: PublicKey, amount: Balance) -> VMContext {
        VMContextBuilder::new()
            .current_account_id(alice())
            .predecessor_account_id(alice())
            .signer_account_id(alice())
            .signer_account_pk(key)
            .account_balance(amount)
            .block_timestamp(1_000_000_000_000_1000)
            .finish()
    }

    #[test]
    fn test_multi_3_of_n() {
        let amount = 1_000;
        testing_env!(context_with_key(
            Base58PublicKey::try_from("Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy")
                .unwrap()
                .into(),
            amount
        ));
        let mut c = MultiSigContract::new(3);
        let request = MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        };
        let request_id = c.add_request(request.clone());
        assert_eq!(c.get_request(request_id), request);
        assert_eq!(c.list_request_ids(), vec![request_id]);
        c.confirm(request_id);
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 1);
        testing_env!(context_with_key(
            Base58PublicKey::try_from("HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R")
                .unwrap()
                .into(),
            amount
        ));
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 2);
        assert_eq!(c.get_confirmations(request_id).len(), 2);
        testing_env!(context_with_key(
            Base58PublicKey::try_from("2EfbwnQHPBWQKbNczLiVznFghh9qs716QT71zN6L1D95")
                .unwrap()
                .into(),
            amount
        ));
        c.confirm(request_id);
        // TODO: confirm that funds were transferred out via promise.
        assert_eq!(c.requests.len(), 0);
    }

    #[test]
    fn test_change_num_confirmations() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![1, 2, 3], amount));
        let mut c = MultiSigContract::new(1);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: alice(),
            actions: vec![MultiSigRequestAction::SetNumConfirmations {
                num_confirmations: 2,
            }],
        });
        c.confirm(request_id);
        assert_eq!(c.num_confirmations, 2);
    }

    #[test]
    #[should_panic]
    fn test_panics_on_second_confirm() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![5, 7, 9], amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 0);
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 1);
        c.confirm(request_id);
    }

    #[test]
    #[should_panic]
    fn test_delete_request_lifespan_panics() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![5, 7, 9], amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        c.delete_request(request_id);
        assert_eq!(c.requests.len(), 0);
        assert_eq!(c.confirmations.len(), 0);
    }

    #[test]
    fn test_expired_request() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![5, 7, 9], amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        c.confirm(request_id);
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.confirmations.len(), 1);
        testing_env!(future_context_with_key(vec![5, 7, 9], amount));
        c.confirm(request_id);
        assert_eq!(c.requests.len(), 0);
        assert_eq!(c.confirmations.len(), 0);
    }

    #[test]
    fn test_delete_request() {
        let amount = 1_000;
        testing_env!(context_with_key(vec![5, 7, 9], amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        testing_env!(future_context_with_key(vec![5, 7, 9], amount));
        c.delete_request(request_id);
        assert_eq!(c.requests.len(), 0);
        assert_eq!(c.confirmations.len(), 0);
    }
}
