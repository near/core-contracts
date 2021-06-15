use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::env::BLOCKCHAIN_INTERFACE;
use near_sdk::json_types::{Base58CryptoHash, Base58PublicKey, Base64VecU8, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Balance, CryptoHash};

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

/// This gas spent on the account creation and contract deployment, the rest goes to the `new` call.
const GAS_FOR_DEPLOY: u64 = 10_000_000_000_000;

const BLOCKCHAIN_INTERFACE_NOT_SET_ERR: &str = "Blockchain interface not set.";

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct GenericFactory {}

impl Default for GenericFactory {
    fn default() -> Self {
        GenericFactory {}
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum AccessKey {
    FullAccessKey(Base58PublicKey),
    FunctionCall {
        public_key: Base58PublicKey,
        receiver_id: AccountId,
        method_names: Vec<String>,
        allowance: U128,
    },
}

#[near_bindgen]
impl GenericFactory {
    pub fn create(
        &self,
        name: AccountId,
        hash: Base58CryptoHash,
        access_keys: Vec<AccessKey>,
        method_name: Option<String>,
        args: Option<Base64VecU8>,
    ) {
        internal_create(
            &format!("{}.{}", name, env::current_account_id()),
            &CryptoHash::from(hash),
            access_keys,
            method_name,
            args.map(|args| args.0),
        );
    }
}

/// Stores attached data into blob store and returns hash of it.
/// Implemented to avoid loading the data into WASM for optimal gas usage.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn store() {
    env::setup_panic_hook();
    env::set_blockchain_interface(Box::new(near_blockchain::NearBlockchain {}));
    unsafe {
        BLOCKCHAIN_INTERFACE.with(|b| {
            // Load input into register 0.
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .input(0);
            // Compute sha256 hash of register 0 and store in 1.
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .sha256(u64::MAX as _, 0 as _, 1);
            // Check if such blob already stored.
            assert_eq!(
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .storage_has_key(u64::MAX as _, 1 as _),
                0,
                "ERR_ALREADY_EXISTS"
            );
            // Get length of the input argument and check that enough $NEAR has been attached.
            let blob_len = b
                .borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .register_len(0);
            let storage_cost = ((blob_len + 32) as u128) * env::storage_byte_cost();
            assert!(
                env::attached_deposit() >= storage_cost,
                "ERR_NOT_ENOUGH_DEPOSIT:{}",
                storage_cost
            );
            // Store value of register 0 into key = register 1.
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .storage_write(u64::MAX as _, 1 as _, u64::MAX as _, 0 as _, 2);
            // Load register 1 into blob_hash and save into LookupMap.
            let blob_hash = [0u8; 32];
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .read_register(1, blob_hash.as_ptr() as _);
            // Return from function value of register 1.
            let blob_hash_str = near_sdk::serde_json::to_string(&Base58CryptoHash::from(blob_hash))
                .unwrap()
                .into_bytes();
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .value_return(blob_hash_str.len() as _, blob_hash_str.as_ptr() as _);
        });
    }
}

pub(crate) fn internal_create(
    account_id: &str,
    hash: &[u8],
    access_keys: Vec<AccessKey>,
    method_name: Option<String>,
    args: Option<Vec<u8>>,
) {
    let attached_deposit = env::attached_deposit();
    unsafe {
        BLOCKCHAIN_INTERFACE.with(|b| {
            // Load input (wasm code) into register 0.
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .storage_read(hash.len() as _, hash.as_ptr() as _, 0);
            // todo: handle missing hash to return attached deposit.
            // schedule a Promise tx to account_id
            let promise_id = b
                .borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .promise_batch_create(account_id.len() as _, account_id.as_ptr() as _);
            // create account first.
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .promise_batch_action_create_account(promise_id);
            // transfer attached deposit.
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .promise_batch_action_transfer(promise_id, &attached_deposit as *const u128 as _);
            // deploy contract (code is taken from register 0).
            b.borrow()
                .as_ref()
                .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                .promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);
            // add access keys.
            for access_key in access_keys.iter() {
                match access_key {
                    AccessKey::FullAccessKey(public_key) => b
                        .borrow()
                        .as_ref()
                        .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                        .promise_batch_action_add_key_with_full_access(
                            promise_id,
                            public_key.0.len() as _,
                            public_key.0.as_ptr() as _,
                            0,
                        ),
                    AccessKey::FunctionCall {
                        public_key,
                        allowance,
                        receiver_id,
                        method_names,
                    } => b
                        .borrow()
                        .as_ref()
                        .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                        .promise_batch_action_add_key_with_function_call(
                            promise_id,
                            public_key.0.len() as _,
                            public_key.0.as_ptr() as _,
                            0,
                            &allowance.0 as *const Balance as _,
                            receiver_id.len() as _,
                            receiver_id.as_ptr() as _,
                            method_names.len() as _,
                            method_names.as_ptr() as _,
                        ),
                }
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_action_deploy_contract(promise_id, u64::MAX as _, 0);
            }
            if method_name.is_some() && args.is_some() {
                // call this_contract.<method_name>(<args>) with remaining gas.
                let attached_gas = env::prepaid_gas() - env::used_gas() - GAS_FOR_DEPLOY;
                let method_name = method_name.unwrap();
                let args = args.unwrap();
                b.borrow()
                    .as_ref()
                    .expect(BLOCKCHAIN_INTERFACE_NOT_SET_ERR)
                    .promise_batch_action_function_call(
                        promise_id,
                        method_name.len() as _,
                        method_name.as_ptr() as _,
                        args.len() as _,
                        args.as_ptr() as _,
                        0,
                        attached_gas,
                    );
                // todo: add callback to handle.
            }
        });
    }
}
