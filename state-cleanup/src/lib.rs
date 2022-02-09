use near_sdk::base64;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json;
use near_sdk::{env, near_bindgen};

#[allow(dead_code)]
#[near_bindgen]
struct Contract {}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct Args {
    pub keys: Vec<String>,
}

#[no_mangle]
pub extern "C" fn clean() {
    env::setup_panic_hook();
    env::set_blockchain_interface(Box::new(near_blockchain::NearBlockchain {}));

    let input = env::input().unwrap();
    let args: Args = serde_json::from_slice(&input).unwrap();
    for key in args.keys.iter() {
        env::storage_remove(&base64::decode(key).unwrap());
    }
}
