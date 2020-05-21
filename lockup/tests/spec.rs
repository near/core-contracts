extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

mod utils;

use lockup_contract::LockupStartInformation;
use near_primitives::types::Balance;
use near_sdk::json_types::{Base58PublicKey, U128};
use utils::{call_lockup, new_root, ntoy, InitLockupArgs};

#[quickcheck]
fn lockup(lockup_amount: Balance, lockup_duration: u64, lockup_timestamp: u64) {
    let (ref mut r, ref mut foundation) = new_root("foundation".into());

    let owner = foundation
        .create_external(r, "owner".into(), ntoy(30))
        .unwrap();

    // let owner_signer = ExternalUser::new(LOCKUP_ACCOUNT_ID.into(), owner.signer().clone());

    let mut owner_raw_public_key = vec![0];
    owner_raw_public_key.append(&mut owner.signer().public_key.unwrap_as_ed25519().0.to_vec());

    let initial_owners_main_public_key = Base58PublicKey(owner_raw_public_key);

    let args = InitLockupArgs {
        lockup_duration: lockup_duration.into(),
        lockup_start_information: LockupStartInformation::TransfersEnabled {
            lockup_timestamp: lockup_timestamp.into(),
        },
        initial_owners_main_public_key,
        foundation_account_id: None,
        owners_staking_public_key: None,
        transfer_poll_account_id: None,
        staking_pool_whitelist_account_id: "staking".into(),
    };

    foundation.init_lockup(r, &args, lockup_amount).unwrap();

    r.current_block().block_timestamp = lockup_timestamp + lockup_duration + 1;

    let locked_amount: U128 = call_lockup(r, "get_locked_amount", "");
    assert_eq!(locked_amount.0, 0);
}
