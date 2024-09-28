use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::serde_json::json;
use near_sdk::{env, near_bindgen, AccountId, Promise};

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

const CODE: &[u8] = include_bytes!("../../multisig/res/multisig.wasm");

/// This gas spent on the call & account creation, the rest goes to the `new` call.
const CREATE_CALL_GAS: u64 = 75_000_000_000_000;

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct MultisigFactory {}

#[near_bindgen]
impl MultisigFactory {
    #[payable]
    pub fn create(
        &mut self,
        name: AccountId,
        members: Vec<Base58PublicKey>,
        num_confirmations: u64,
    ) -> Promise {
        let account_id = format!("{}.{}", name, env::current_account_id());
        let mut promise = Promise::new(account_id.clone())
            .create_account()
            .deploy_contract(CODE.to_vec())
            .transfer(env::attached_deposit());

        // Add access keys for each member
        for member in &members {
            promise = promise.add_access_key(
                member.clone().into(),
                0,
                account_id.clone(),
                b"add_request,add_request_and_confirm,delete_request,confirm".to_vec(),
            );
        }

        promise.function_call(
            b"new".to_vec(),
            json!({ "num_confirmations": num_confirmations })
                .to_string()
                .as_bytes()
                .to_vec(),
            0,
            env::prepaid_gas() - CREATE_CALL_GAS,
        )
    }
}
