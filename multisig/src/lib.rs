use std::collections::BTreeSet;
use std::convert::TryFrom;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Gas, Promise, PromiseOrValue, PublicKey};

/// Unlimited allowance for multisig keys.
const DEFAULT_ALLOWANCE: u128 = 0;

// Request cooldown period (time before a request can be deleted)
const REQUEST_COOLDOWN: u64 = 900_000_000_000;

pub type RequestId = u32;

/// Permissions for function call access key.
#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct FunctionCallPermission {
    allowance: Option<U128>,
    receiver_id: AccountId,
    method_names: Vec<String>,
}

/// Lowest level action that can be performed by the multisig contract.
#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(tag = "type", crate = "near_sdk::serde")]
pub enum MultiSigRequestAction {
    /// Transfers given amount to receiver.
    Transfer { amount: U128 },
    /// Create a new account.
    CreateAccount,
    /// Deploys contract to receiver's account. Can upgrade given contract as well.
    DeployContract { code: Base64VecU8 },
    /// Adds key, either new key for multisig or full access key to another account.
    AddKey {
        public_key: PublicKey,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission: Option<FunctionCallPermission>,
    },
    /// Deletes key, either one of the keys from multisig or key from another account.
    DeleteKey { public_key: PublicKey },
    /// Call function on behalf of this contract.
    FunctionCall {
        method_name: String,
        args: Base64VecU8,
        deposit: U128,
        gas: Gas,
    },
    /// Sets number of confirmations required to authorize requests.
    /// Can not be bundled with any other actions or transactions.
    SetNumConfirmations { num_confirmations: u32 },
    /// Sets number of active requests (unconfirmed requests) per access key
    /// Default is 12 unconfirmed requests at a time
    /// The REQUEST_COOLDOWN for requests is 15min
    /// Worst gas attack a malicious keyholder could do is 12 requests every 15min
    SetActiveRequestsLimit { active_requests_limit: u32 },
}

// The request the user makes specifying the receiving account and actions they want to execute (1 tx)
#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MultiSigRequest {
    receiver_id: AccountId,
    actions: Vec<MultiSigRequestAction>,
}

// An internal request wrapped with the signer_pk and added timestamp to determine num_requests_pk and prevent against malicious key holder gas attacks
#[derive(Clone, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MultiSigRequestWithSigner {
    request: MultiSigRequest,
    signer_pk: PublicKey,
    added_timestamp: u64,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct MultiSigContract {
    num_confirmations: u32,
    request_nonce: RequestId,
    requests: UnorderedMap<RequestId, MultiSigRequestWithSigner>,
    confirmations: UnorderedMap<RequestId, BTreeSet<PublicKey>>,
    num_requests_pk: UnorderedMap<PublicKey, u32>,
    // per key
    active_requests_limit: u32,
}

// If you haven't initialized the contract with new(num_confirmations: u32)
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
            requests: UnorderedMap::new(b"r".to_vec()),
            confirmations: UnorderedMap::new(b"c".to_vec()),
            num_requests_pk: UnorderedMap::new(b"k".to_vec()),
            active_requests_limit: 12,
        }
    }

    /// Add request for multisig.
    pub fn add_request(&mut self, request: MultiSigRequest) -> RequestId {
        assert_eq!(
            env::current_account_id(),
            env::predecessor_account_id(),
            "Predecessor account must much current account"
        );
        // track how many requests this key has made
        let num_requests = self
            .num_requests_pk
            .get(&env::signer_account_pk())
            .unwrap_or(0)
            + 1;
        assert!(
            num_requests <= self.active_requests_limit,
            "Account has too many active requests. Confirm or delete some."
        );
        self.num_requests_pk
            .insert(&env::signer_account_pk(), &num_requests);
        // add the request
        let request_added = MultiSigRequestWithSigner {
            signer_pk: env::signer_account_pk(),
            added_timestamp: env::block_timestamp(),
            request: request,
        };
        self.requests.insert(&self.request_nonce, &request_added);
        let confirmations = BTreeSet::new();
        self.confirmations
            .insert(&self.request_nonce, &confirmations);
        self.request_nonce += 1;
        self.request_nonce - 1
    }

    /// Add request for multisig and confirm with the pk that added.
    pub fn add_request_and_confirm(&mut self, request: MultiSigRequest) -> RequestId {
        let request_id = self.add_request(request);
        self.confirm(request_id);
        request_id
    }

    /// Remove given request and associated confirmations.
    pub fn delete_request(&mut self, request_id: RequestId) {
        self.assert_valid_request(request_id);
        let request_with_signer = self.requests.get(&request_id).expect("No such request");
        // can't delete requests before 15min
        assert!(
            env::block_timestamp() > request_with_signer.added_timestamp + REQUEST_COOLDOWN,
            "Request cannot be deleted immediately after creation."
        );
        self.remove_request(request_id);
    }

    fn execute_request(&mut self, request: MultiSigRequest) -> PromiseOrValue<bool> {
        let mut promise = Promise::new(request.receiver_id.clone());
        let receiver_id = request.receiver_id.clone();
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
                } => {
                    self.assert_self_request(receiver_id.clone());
                    if let Some(permission) = permission {
                        promise.add_access_key(
                            public_key.into(),
                            permission
                                .allowance
                                .map(|x| x.into())
                                .unwrap_or(DEFAULT_ALLOWANCE),
                            permission.receiver_id,
                            permission.method_names.join(",").into_bytes(),
                        )
                    } else {
                        // wallet UI should warn user if receiver_id == env::current_account_id(), adding FAK will render multisig useless
                        promise.add_full_access_key(public_key.into())
                    }
                }
                MultiSigRequestAction::DeleteKey { public_key } => {
                    self.assert_self_request(receiver_id.clone());
                    let pk: PublicKey = public_key.into();
                    // delete outstanding requests by public_key
                    let request_ids: Vec<u32> = self
                        .requests
                        .iter()
                        .filter(|(_k, r)| r.signer_pk == pk)
                        .map(|(k, _r)| k)
                        .collect();
                    for request_id in request_ids {
                        // remove confirmations for this request
                        self.confirmations.remove(&request_id);
                        self.requests.remove(&request_id);
                    }
                    // remove num_requests_pk entry for public_key
                    self.num_requests_pk.remove(&pk);
                    promise.delete_key(pk)
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
                    gas,
                ),
                // the following methods must be a single action
                MultiSigRequestAction::SetNumConfirmations { num_confirmations } => {
                    self.assert_one_action_only(receiver_id, num_actions);
                    self.num_confirmations = num_confirmations;
                    return PromiseOrValue::Value(true);
                }
                MultiSigRequestAction::SetActiveRequestsLimit {
                    active_requests_limit,
                } => {
                    self.assert_one_action_only(receiver_id, num_actions);
                    self.active_requests_limit = active_requests_limit;
                    return PromiseOrValue::Value(true);
                }
            };
        }
        promise.into()
    }

    /// Confirm given request with given signing key.
    /// If with this, there has been enough confirmation, a promise with request will be scheduled.
    pub fn confirm(&mut self, request_id: RequestId) -> PromiseOrValue<bool> {
        self.assert_valid_request(request_id);
        let mut confirmations = self.confirmations.get(&request_id).unwrap();
        assert!(
            !confirmations.contains(&env::signer_account_pk()),
            "Already confirmed this request with this key"
        );
        if confirmations.len() as u32 + 1 >= self.num_confirmations {
            let request = self.remove_request(request_id);
            /********************************
            NOTE: If the tx execution fails for any reason, the request and confirmations are removed already, so the client has to start all over
            ********************************/
            self.execute_request(request)
        } else {
            confirmations.insert(env::signer_account_pk());
            self.confirmations.insert(&request_id, &confirmations);
            PromiseOrValue::Value(true)
        }
    }

    /********************************
    Helper methods
    ********************************/
    // removes request, removes confirmations and reduces num_requests_pk - used in delete, delete_key, and confirm
    fn remove_request(&mut self, request_id: RequestId) -> MultiSigRequest {
        // remove confirmations for this request
        self.confirmations.remove(&request_id);
        // remove the original request
        let request_with_signer = self
            .requests
            .remove(&request_id)
            .expect("Failed to remove existing element");
        // decrement num_requests for original request signer
        let original_signer_pk = request_with_signer.signer_pk;
        let mut num_requests = self.num_requests_pk.get(&original_signer_pk).unwrap_or(0);
        // safety check for underrun (unlikely since original_signer_pk must have num_requests_pk > 0)
        if num_requests > 0 {
            num_requests = num_requests - 1;
        }
        self.num_requests_pk
            .insert(&original_signer_pk, &num_requests);
        // return request
        request_with_signer.request
    }
    // Prevents access to calling requests and make sure request_id is valid - used in delete and confirm
    fn assert_valid_request(&mut self, request_id: RequestId) {
        // request must come from key added to contract account
        assert_eq!(
            env::current_account_id(),
            env::predecessor_account_id(),
            "Predecessor account must much current account"
        );
        // request must exist
        assert!(
            self.requests.get(&request_id).is_some(),
            "No such request: either wrong number or already confirmed"
        );
        // request must have
        assert!(
            self.confirmations.get(&request_id).is_some(),
            "Internal error: confirmations mismatch requests"
        );
    }
    // Prevents request from approving tx on another account
    fn assert_self_request(&mut self, receiver_id: AccountId) {
        assert_eq!(
            receiver_id,
            env::current_account_id(),
            "This method only works when receiver_id is equal to current_account_id"
        );
    }
    // Prevents a request from being bundled with other actions
    fn assert_one_action_only(&mut self, receiver_id: AccountId, num_actions: usize) {
        self.assert_self_request(receiver_id);
        assert_eq!(num_actions, 1, "This method should be a separate request");
    }
    /********************************
    View methods
    ********************************/
    pub fn get_request(&self, request_id: RequestId) -> MultiSigRequest {
        (self.requests.get(&request_id).expect("No such request")).request
    }

    pub fn get_num_requests_pk(&self, public_key: PublicKey) -> u32 {
        self.num_requests_pk.get(&public_key.into()).unwrap_or(0)
    }

    pub fn list_request_ids(&self) -> Vec<RequestId> {
        self.requests.keys().collect()
    }

    pub fn get_confirmations(&self, request_id: RequestId) -> Vec<PublicKey> {
        self.confirmations
            .get(&request_id)
            .expect("No such request")
            .into_iter()
            .map(|key| PublicKey::try_from(key).expect("Failed to covert key to base58"))
            .collect()
    }

    pub fn get_num_confirmations(&self) -> u32 {
        self.num_confirmations
    }

    pub fn get_request_nonce(&self) -> u32 {
        self.request_nonce
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::{Debug, Error, Formatter};

    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;
    use near_sdk::Balance;
    use near_sdk::{AccountId, VMContext};

    use super::*;

    /// Used for asserts_eq.
    /// TODO: replace with derive when https://github.com/near/near-sdk-rs/issues/165
    impl Debug for MultiSigRequest {
        fn fmt(&self, _f: &mut Formatter<'_>) -> Result<(), Error> {
            panic!("Should not trigger");
        }
    }

    pub fn alice() -> AccountId {
        "alice".parse().unwrap()
    }
    pub fn bob() -> AccountId {
        "bob".parse().unwrap()
    }

    fn context_with_key(key: PublicKey, amount: Balance) -> VMContext {
        VMContextBuilder::new()
            .current_account_id(alice())
            .predecessor_account_id(alice())
            .signer_account_id(alice())
            .signer_account_pk(key)
            .account_balance(amount)
            .build()
    }

    fn dummy_public_key() -> PublicKey {
        "Eg2jtsiMrprn7zgaKUk79qM1hWhANsFyE6JSX4txLEub"
            .parse()
            .unwrap()
    }

    fn context_with_key_future(key: PublicKey, amount: Balance) -> VMContext {
        VMContextBuilder::new()
            .current_account_id(alice())
            .block_timestamp(REQUEST_COOLDOWN + 1)
            .predecessor_account_id(alice())
            .signer_account_id(alice())
            .signer_account_pk(key)
            .account_balance(amount)
            .build()
    }

    #[test]
    fn test_multi_3_of_n() {
        let amount = 1_000;
        testing_env!(context_with_key(
            "Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                .parse()
                .unwrap(),
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
            "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                .parse()
                .unwrap(),
            amount
        ));
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 2);
        assert_eq!(c.get_confirmations(request_id).len(), 2);
        testing_env!(context_with_key(
            "2EfbwnQHPBWQKbNczLiVznFghh9qs716QT71zN6L1D95"
                .parse()
                .unwrap(),
            amount
        ));
        c.confirm(request_id);
        // TODO: confirm that funds were transferred out via promise.
        assert_eq!(c.requests.len(), 0);
    }

    #[test]
    fn test_multi_add_request_and_confirm() {
        let amount = 1_000;
        testing_env!(context_with_key(
            "Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                .parse()
                .unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(3);
        let request = MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        };
        let request_id = c.add_request_and_confirm(request.clone());
        assert_eq!(c.get_request(request_id), request);
        assert_eq!(c.list_request_ids(), vec![request_id]);
        // c.confirm(request_id);
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 1);
        testing_env!(context_with_key(
            "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                .parse()
                .unwrap(),
            amount
        ));
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 2);
        assert_eq!(c.get_confirmations(request_id).len(), 2);
        testing_env!(context_with_key(
            "2EfbwnQHPBWQKbNczLiVznFghh9qs716QT71zN6L1D95"
                .parse()
                .unwrap(),
            amount
        ));
        c.confirm(request_id);
        // TODO: confirm that funds were transferred out via promise.
        assert_eq!(c.requests.len(), 0);
    }

    #[test]
    fn add_key_delete_key_storage_cleared() {
        let amount = 1_000;
        testing_env!(context_with_key(
            "Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                .parse()
                .unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(1);
        let new_key: PublicKey = "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
            .parse()
            .unwrap();
        // vm current_account_id is alice, receiver_id must be alice
        let request = MultiSigRequest {
            receiver_id: alice(),
            actions: vec![MultiSigRequestAction::AddKey {
                public_key: new_key.clone(),
                permission: None,
            }],
        };
        // make request
        c.add_request_and_confirm(request.clone());
        // should be empty now
        assert_eq!(c.requests.len(), 0);
        // switch accounts
        testing_env!(context_with_key(
            "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                .parse()
                .unwrap(),
            amount
        ));
        let request2 = MultiSigRequest {
            receiver_id: alice(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        };
        // make request but don't confirm
        c.add_request(request2.clone());
        // should have 1 request now
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.get_num_requests_pk(new_key.clone()), 1);
        // self delete key
        let request3 = MultiSigRequest {
            receiver_id: alice(),
            actions: vec![MultiSigRequestAction::DeleteKey {
                public_key: new_key.clone(),
            }],
        };
        // make request and confirm
        c.add_request_and_confirm(request3.clone());
        // should be empty now
        assert_eq!(c.requests.len(), 0);
        assert_eq!(c.get_num_requests_pk(new_key.clone()), 0);
    }

    #[test]
    #[should_panic]
    fn test_panics_add_key_different_account() {
        let amount = 1_000;
        testing_env!(context_with_key(
            "Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                .parse()
                .unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(1);
        let new_key: PublicKey = "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
            .parse()
            .unwrap();
        // vm current_account_id is alice, receiver_id must be alice
        let request = MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::AddKey {
                public_key: new_key.clone(),
                permission: None,
            }],
        };
        // make request
        c.add_request_and_confirm(request);
    }

    #[test]
    fn test_change_num_confirmations() {
        let amount = 1_000;
        testing_env!(context_with_key(dummy_public_key(), amount));
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
        testing_env!(context_with_key(dummy_public_key(), amount));
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
    fn test_panics_delete_request() {
        let amount = 1_000;
        testing_env!(context_with_key(dummy_public_key(), amount));
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
    fn test_delete_request_future() {
        let amount = 1_000;
        testing_env!(context_with_key(dummy_public_key(), amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        testing_env!(context_with_key_future(dummy_public_key(), amount));
        c.delete_request(request_id);
        assert_eq!(c.requests.len(), 0);
        assert_eq!(c.confirmations.len(), 0);
    }

    #[test]
    #[should_panic]
    fn test_delete_request_panic_wrong_key() {
        let amount = 1_000;
        testing_env!(context_with_key(dummy_public_key(), amount));
        let mut c = MultiSigContract::new(3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        testing_env!(context_with_key(dummy_public_key(), amount));
        c.delete_request(request_id);
    }

    #[test]
    #[should_panic]
    fn test_too_many_requests() {
        let amount = 1_000;
        testing_env!(context_with_key(dummy_public_key(), amount));
        let mut c = MultiSigContract::new(3);
        for _i in 0..16 {
            c.add_request(MultiSigRequest {
                receiver_id: bob(),
                actions: vec![MultiSigRequestAction::Transfer {
                    amount: amount.into(),
                }],
            });
        }
    }
}
