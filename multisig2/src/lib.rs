use std::collections::HashSet;
use std::convert::TryInto;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, near_bindgen, serde_json, AccountId, BorshStorageKey, Gas, PanicOnDefault, Promise,
    PromiseOrValue, PublicKey,
};

/// Unlimited allowance for multisig keys.
const DEFAULT_ALLOWANCE: u128 = 0;

/// Request cooldown period (time before a request can be deleted)
const REQUEST_COOLDOWN: u64 = 900_000_000_000;

/// Default limit of active requests.
const ACTIVE_REQUESTS_LIMIT: u32 = 12;

/// Default set of methods that access key should have.
const MULTISIG_METHOD_NAMES: &str = "add_request,delete_request,confirm,add_and_confirm_request";

pub type RequestId = u32;

/// Permissions for function call access key.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Clone))]
#[serde(crate = "near_sdk::serde")]
pub struct FunctionCallPermission {
    allowance: Option<U128>,
    receiver_id: AccountId,
    method_names: Vec<String>,
}

/// Lowest level action that can be performed by the multisig contract.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Clone))]
#[serde(tag = "type", crate = "near_sdk::serde")]
pub enum MultiSigRequestAction {
    /// Transfers given amount to receiver.
    Transfer { amount: U128 },
    /// Create a new account.
    CreateAccount,
    /// Deploys contract to receiver's account. Can upgrade given contract as well.
    DeployContract { code: Base64VecU8 },
    /// Add new member of the multisig.
    AddMember { member: MultisigMember },
    /// Remove existing member of the multisig.
    DeleteMember { member: MultisigMember },
    /// Adds full access key to another account.
    AddKey {
        public_key: PublicKey,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission: Option<FunctionCallPermission>,
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
    SetNumConfirmations { num_confirmations: u32 },
    /// Sets number of active requests (unconfirmed requests) per access key
    /// Default is 12 unconfirmed requests at a time
    /// The REQUEST_COOLDOWN for requests is 15min
    /// Worst gas attack a malicious keyholder could do is 12 requests every 15min
    SetActiveRequestsLimit { active_requests_limit: u32 },
}

/// The request the user makes specifying the receiving account and actions they want to execute (1 tx)
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Clone))]
#[serde(crate = "near_sdk::serde")]
pub struct MultiSigRequest {
    receiver_id: AccountId,
    actions: Vec<MultiSigRequestAction>,
}

/// An internal request wrapped with the signer_pk and added timestamp to determine num_requests_pk and prevent against malicious key holder gas attacks
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[cfg_attr(test, derive(PartialEq, Clone))]
#[serde(crate = "near_sdk::serde")]
pub struct MultiSigRequestWithSigner {
    request: MultiSigRequest,
    member: MultisigMember,
    added_timestamp: u64,
}

/// Represents member of the multsig: either account or access key to given account.
#[derive(Debug, BorshDeserialize, BorshSerialize, Clone, PartialEq, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde", untagged)]
pub enum MultisigMember {
    AccessKey { public_key: PublicKey },
    Account { account_id: AccountId },
}

impl ToString for MultisigMember {
    fn to_string(&self) -> String {
        serde_json::to_string(&self).unwrap_or_else(|_| env::panic_str("Failed to serialize"))
    }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub enum StorageKeys {
    Members,
    Requests,
    Confirmations,
    NumRequestsPk,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MultiSigContract {
    /// Members of the multisig.
    members: UnorderedSet<MultisigMember>,
    /// Number of confirmations required.
    num_confirmations: u32,
    /// Latest request nonce.
    request_nonce: RequestId,
    /// All active requests.
    requests: UnorderedMap<RequestId, MultiSigRequestWithSigner>,
    /// All confirmations for active requests.
    confirmations: LookupMap<RequestId, HashSet<String>>,
    /// Number of requests per member.
    num_requests_pk: LookupMap<String, u32>,
    /// Limit number of active requests per member.
    active_requests_limit: u32,
}

#[inline]
fn assert(condition: bool, error: &str) {
    if !condition {
        env::panic_str(error);
    }
}

#[near_bindgen]
impl MultiSigContract {
    /// Initialize multisig contract.
    /// @params members: list of {"account_id": "name"} or {"public_key": "key"} members.
    /// @params num_confirmations: k of n signatures required to perform operations.
    #[init]
    pub fn new(members: Vec<MultisigMember>, num_confirmations: u32) -> Self {
        assert(
            members.len() >= num_confirmations as usize,
            "Members list must be equal or larger than number of confirmations",
        );
        let mut multisig = Self {
            members: UnorderedSet::new(StorageKeys::Members),
            num_confirmations,
            request_nonce: 0,
            requests: UnorderedMap::new(StorageKeys::Requests),
            confirmations: LookupMap::new(StorageKeys::Confirmations),
            num_requests_pk: LookupMap::new(StorageKeys::NumRequestsPk),
            active_requests_limit: ACTIVE_REQUESTS_LIMIT,
        };
        let mut promise = Promise::new(env::current_account_id());
        for member in members {
            promise = multisig.add_member(promise, member);
        }
        multisig
    }

    /// Add request for multisig.
    pub fn add_request(&mut self, request: MultiSigRequest) -> RequestId {
        let current_member = self.current_member().unwrap_or_else(|| {
            env::panic_str(
                "Predecessor must be a member or transaction signed with key of given account",
            )
        });
        // track how many requests this key has made
        let num_requests = self
            .num_requests_pk
            .get(&current_member.to_string())
            .unwrap_or(0)
            + 1;
        assert(
            num_requests <= self.active_requests_limit,
            "Account has too many active requests. Confirm or delete some.",
        );
        self.num_requests_pk
            .insert(&current_member.to_string(), &num_requests);
        // add the request
        let request_added = MultiSigRequestWithSigner {
            member: current_member,
            added_timestamp: env::block_timestamp(),
            request,
        };
        self.requests.insert(&self.request_nonce, &request_added);
        let confirmations = HashSet::new();
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
        let request_with_signer = self
            .requests
            .get(&request_id)
            .unwrap_or_else(|| env::panic_str("No such request"));
        // can't delete requests before 15min
        assert(
            env::block_timestamp() > request_with_signer.added_timestamp + REQUEST_COOLDOWN,
            "Request cannot be deleted immediately after creation.",
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
                MultiSigRequestAction::AddMember { member } => {
                    self.assert_self_request(receiver_id.clone());
                    self.add_member(promise, member)
                }
                MultiSigRequestAction::DeleteMember { member } => {
                    self.assert_self_request(receiver_id.clone());
                    self.delete_member(promise, member)
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
                            permission.method_names.join(","),
                        )
                    } else {
                        // wallet UI should warn user if receiver_id == env::current_account_id(), adding FAK will render multisig useless
                        promise.add_full_access_key(public_key.into())
                    }
                }
                MultiSigRequestAction::FunctionCall {
                    method_name,
                    args,
                    deposit,
                    gas,
                } => promise.function_call(
                    method_name,
                    args.into(),
                    deposit.into(),
                    Gas::from(gas.0),
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
        let member = self
            .current_member()
            .unwrap_or_else(|| env::panic_str("Must be validated above"));
        let mut confirmations = self.confirmations.get(&request_id).unwrap();
        assert(
            !confirmations.contains(&member.to_string()),
            "Already confirmed this request with this key",
        );
        if confirmations.len() as u32 + 1 >= self.num_confirmations {
            let request = self.remove_request(request_id);
            /********************************
            NOTE: If the tx execution fails for any reason, the request and confirmations are removed already, so the client has to start all over
            ********************************/
            self.execute_request(request)
        } else {
            confirmations.insert(member.to_string());
            self.confirmations.insert(&request_id, &confirmations);
            PromiseOrValue::Value(true)
        }
    }

    /********************************
    Helper methods
    ********************************/

    /// Returns current member: either predecessor as account or if it's the same as current account - signer.
    fn current_member(&self) -> Option<MultisigMember> {
        let member = if env::current_account_id() == env::predecessor_account_id() {
            MultisigMember::AccessKey {
                public_key: env::signer_account_pk()
                    .try_into()
                    .unwrap_or_else(|_| env::panic_str("Failed to deserialize public key")),
            }
        } else {
            MultisigMember::Account {
                account_id: env::predecessor_account_id(),
            }
        };
        if self.members.contains(&member) {
            Some(member)
        } else {
            None
        }
    }

    /// Add member to the list. Adds access key if member is key based.
    fn add_member(&mut self, promise: Promise, member: MultisigMember) -> Promise {
        self.members.insert(&member.clone().into());
        match member {
            MultisigMember::AccessKey { public_key } => promise.add_access_key(
                public_key.into(),
                DEFAULT_ALLOWANCE,
                env::current_account_id(),
                MULTISIG_METHOD_NAMES.to_string(),
            ),
            MultisigMember::Account { account_id: _ } => promise,
        }
    }

    /// Delete member from the list. Removes access key if the member is key based.
    fn delete_member(&mut self, promise: Promise, member: MultisigMember) -> Promise {
        assert(
            self.members.len() - 1 >= self.num_confirmations as u64,
            "Removing given member will make total number of members below number of confirmations",
        );
        // delete outstanding requests by public_key
        let request_ids: Vec<u32> = self
            .requests
            .iter()
            .filter_map(|(k, r)| if r.member == member { Some(k) } else { None })
            .collect();
        for request_id in request_ids {
            // remove confirmations for this request
            self.confirmations.remove(&request_id);
            self.requests.remove(&request_id);
        }
        // remove num_requests_pk entry for member
        self.num_requests_pk.remove(&member.to_string());
        self.members.remove(&member);
        match member {
            MultisigMember::AccessKey { public_key } => promise.delete_key(public_key.into()),
            MultisigMember::Account { account_id: _ } => promise,
        }
    }

    /// Removes request, removes confirmations and reduces num_requests_pk - used in delete, delete_key, and confirm
    fn remove_request(&mut self, request_id: RequestId) -> MultiSigRequest {
        // remove confirmations for this request
        self.confirmations.remove(&request_id);
        // remove the original request
        let request_with_signer = self
            .requests
            .remove(&request_id)
            .unwrap_or_else(|| env::panic_str("Failed to remove existing element"));
        // decrement num_requests for original request signer
        let original_member = request_with_signer.member;
        let mut num_requests = self
            .num_requests_pk
            .get(&original_member.to_string())
            .unwrap_or(0);
        // safety check for underrun (unlikely since original_signer_pk must have num_requests_pk > 0)
        if num_requests > 0 {
            num_requests = num_requests - 1;
        }
        self.num_requests_pk
            .insert(&original_member.to_string(), &num_requests);
        // return request
        request_with_signer.request
    }

    /// Prevents access to calling requests and make sure request_id is valid - used in delete and confirm
    fn assert_valid_request(&mut self, request_id: RequestId) {
        // request must come from key added to contract account
        assert(
            self.current_member().is_some(),
            "Caller (predecessor or signer) is not a member of this multisig",
        );
        // request must exist
        assert(
            self.requests.get(&request_id).is_some(),
            "No such request: either wrong number or already confirmed",
        );
        // request must have
        assert(
            self.confirmations.get(&request_id).is_some(),
            "Internal error: confirmations mismatch requests",
        );
    }

    /// Prevents request from approving tx on another account
    fn assert_self_request(&mut self, receiver_id: AccountId) {
        assert(
            receiver_id == env::current_account_id(),
            "This method only works when receiver_id is equal to current_account_id",
        );
    }

    /// Prevents a request from being bundled with other actions
    fn assert_one_action_only(&mut self, receiver_id: AccountId, num_actions: usize) {
        self.assert_self_request(receiver_id);
        assert(num_actions == 1, "This method should be a separate request");
    }

    /********************************
    View methods
    ********************************/

    /// Returns members of the multisig.
    pub fn get_members(&self) -> Vec<MultisigMember> {
        self.members.to_vec()
    }

    pub fn get_request(&self, request_id: RequestId) -> MultiSigRequest {
        (self
            .requests
            .get(&request_id)
            .unwrap_or_else(|| env::panic_str("No such request")))
        .request
    }

    pub fn get_num_requests_per_member(&self, member: MultisigMember) -> u32 {
        self.num_requests_pk.get(&member.to_string()).unwrap_or(0)
    }

    pub fn list_request_ids(&self) -> Vec<RequestId> {
        self.requests.keys().collect()
    }

    pub fn get_confirmations(&self, request_id: RequestId) -> Vec<String> {
        self.confirmations
            .get(&request_id)
            .unwrap_or_else(|| env::panic_str("No such request"))
            .into_iter()
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
    use std::convert::TryFrom;
    use std::fmt::{Debug, Error, Formatter};

    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::Balance;
    use near_sdk::{testing_env, PublicKey};
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
        AccountId::new_unchecked("alice".to_string())
    }
    pub fn bob() -> AccountId {
        AccountId::new_unchecked("bob".to_string())
    }

    const TEST_KEY: [u8; 33] = [
        0, 247, 230, 176, 93, 224, 175, 33, 211, 72, 124, 12, 163, 219, 7, 137, 3, 37, 162, 199,
        181, 38, 90, 244, 111, 207, 37, 216, 79, 84, 50, 83, 164,
    ];

    fn members() -> Vec<MultisigMember> {
        vec![
            MultisigMember::Account {
                account_id: alice(),
            },
            MultisigMember::Account { account_id: bob() },
            MultisigMember::AccessKey {
                public_key: PublicKey::from(
                    "ed25519:Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                        .parse()
                        .unwrap(),
                ),
            },
            MultisigMember::AccessKey {
                public_key: PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            },
        ]
    }

    fn context_with_key(key: PublicKey, amount: Balance) -> VMContext {
        context_with_account_key(alice(), key, amount)
    }

    fn context_with_account(account_id: AccountId, amount: Balance) -> VMContext {
        context_with_account_key(
            account_id,
            PublicKey::try_from(vec![
                0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32, 33,
            ])
            .unwrap(),
            amount,
        )
    }

    fn context_with_account_key(
        account_id: AccountId,
        key: PublicKey,
        amount: Balance,
    ) -> VMContext {
        VMContextBuilder::new()
            .current_account_id(alice())
            .predecessor_account_id(account_id.clone())
            .signer_account_id(account_id.clone())
            .signer_account_pk(key)
            .account_balance(amount)
            .build()
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
            PublicKey::from(
                "Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                    .parse()
                    .unwrap()
            ),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
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
            PublicKey::from(
                "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                    .parse()
                    .unwrap()
            ),
            amount
        ));
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 2);
        assert_eq!(c.get_confirmations(request_id).len(), 2);
        testing_env!(context_with_account(bob(), amount));
        c.confirm(request_id);
        // TODO: confirm that funds were transferred out via promise.
        assert_eq!(c.requests.len(), 0);
    }

    #[test]
    fn test_multi_add_request_and_confirm() {
        let amount = 1_000;
        testing_env!(context_with_key(
            PublicKey::from(
                "Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                    .parse()
                    .unwrap()
            ),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
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
            PublicKey::from(
                "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                    .parse()
                    .unwrap()
            ),
            amount
        ));
        c.confirm(request_id);
        assert_eq!(c.confirmations.get(&request_id).unwrap().len(), 2);
        assert_eq!(c.get_confirmations(request_id).len(), 2);
        testing_env!(context_with_account(bob(), amount));
        c.confirm(request_id);
        // TODO: confirm that funds were transferred out via promise.
        assert_eq!(c.requests.len(), 0);
    }

    #[test]
    fn add_key_delete_key_storage_cleared() {
        let amount = 1_000;
        testing_env!(context_with_key(
            PublicKey::from(
                "ed25519:Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"
                    .parse()
                    .unwrap()
            ),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 1);
        let new_key: PublicKey = PublicKey::from(
            "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                .parse()
                .unwrap(),
        );
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
            PublicKey::from(
                "HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"
                    .parse()
                    .unwrap()
            ),
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
        let new_member = MultisigMember::AccessKey {
            public_key: new_key.clone(),
        };
        assert_eq!(c.requests.len(), 1);
        assert_eq!(c.get_num_requests_per_member(new_member.clone()), 1);
        // self delete key
        let request3 = MultiSigRequest {
            receiver_id: alice(),
            actions: vec![MultiSigRequestAction::DeleteMember {
                member: new_member.clone(),
            }],
        };
        // make request and confirm
        c.add_request_and_confirm(request3.clone());
        // should be empty now
        assert_eq!(c.requests.len(), 0);
        assert_eq!(c.get_num_requests_per_member(new_member), 0);
    }

    #[test]
    #[should_panic]
    fn test_panics_add_key_different_account() {
        let amount = 1_000;
        testing_env!(context_with_key(
            PublicKey::try_from(Vec::from("Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy")).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 1);
        let new_key: PublicKey =
            PublicKey::try_from(Vec::from("HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"))
                .unwrap()
                .into();
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
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 1);
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
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
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
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        c.delete_request(request_id);
    }

    #[test]
    fn test_delete_request_future() {
        let amount = 1_000;
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        c.confirm(request_id);
        testing_env!(context_with_key_future(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        c.delete_request(request_id);
        assert_eq!(c.requests.len(), 0);
        assert!(c.confirmations.get(&request_id).is_none());
    }

    #[test]
    #[should_panic]
    fn test_delete_request_panic_wrong_key() {
        let amount = 1_000;
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
        let request_id = c.add_request(MultiSigRequest {
            receiver_id: bob(),
            actions: vec![MultiSigRequestAction::Transfer {
                amount: amount.into(),
            }],
        });
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        c.delete_request(request_id);
    }

    #[test]
    #[should_panic]
    fn test_too_many_requests() {
        let amount = 1_000;
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            amount
        ));
        let mut c = MultiSigContract::new(members(), 3);
        for _i in 0..16 {
            c.add_request(MultiSigRequest {
                receiver_id: bob(),
                actions: vec![MultiSigRequestAction::Transfer {
                    amount: amount.into(),
                }],
            });
        }
    }

    #[test]
    #[should_panic]
    fn test_too_many_confirmations() {
        testing_env!(context_with_key(
            PublicKey::try_from(TEST_KEY.to_vec()).unwrap(),
            1_000
        ));
        let _ = MultiSigContract::new(members(), 5);
    }
}
