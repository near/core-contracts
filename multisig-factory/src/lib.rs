use std::convert::TryFrom;

use near_sdk::json_types::Base58PublicKey;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::json;
use near_sdk::{env, near, AccountId, Gas, GasWeight, NearToken, Promise};

const CODE: &[u8] = include_bytes!("../../multisig2/res/multisig2.wasm");

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde", untagged)]
pub enum MultisigMember {
    AccessKey { public_key: Base58PublicKey },
    Account { account_id: AccountId },
}

#[derive(Default)]
#[near(contract_state)]
pub struct MultisigFactory {}

#[near]
impl MultisigFactory {
    #[payable]
    pub fn create(
        &mut self,
        name: AccountId,
        members: Vec<MultisigMember>,
        num_confirmations: u64,
    ) -> Promise {
        let account_id =
            AccountId::try_from(format!("{}.{}", name, env::current_account_id())).unwrap();
        Promise::new(account_id)
            .create_account()
            .deploy_contract(CODE.to_vec())
            .transfer(env::attached_deposit())
            .function_call_weight(
                "new".to_string(),
                json!({ "members": members, "num_confirmations": num_confirmations })
                    .to_string()
                    .as_bytes()
                    .to_vec(),
                NearToken::from_yoctonear(0),
                Gas::from_tgas(15),
                GasWeight(1),
            )
    }
}
