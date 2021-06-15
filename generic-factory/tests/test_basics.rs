use near_sdk::json_types::Base64VecU8;
use near_sdk::serde_json;
use near_sdk_sim::{init_simulator, to_yocto};

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    WASM_BYTES => "res/generic_factory.wasm",
    MULTISIG_BYTES => "../multisig/res/multisig.wasm",
}

#[test]
fn test_upload_create() {
    let root = init_simulator(None);
    let factory = root.deploy(&WASM_BYTES, "xyz".to_string(), to_yocto("10"));
    let hash = root
        .call(
            factory.account_id.clone(),
            "store",
            &MULTISIG_BYTES,
            near_sdk_sim::DEFAULT_GAS,
            to_yocto("10"),
        )
        .unwrap_json::<String>();
    let args = serde_json::to_string(&Base64VecU8::from(
        "{\"num_confirmations\": 2}".as_bytes().to_vec(),
    ))
    .unwrap();
    let all_args = format!(
        "{{\"name\": \"test\", \"hash\": \"{}\", \"method_name\": \"new\", \"args\": {}, \"access_keys\": [\"H8qfQA4p5T4rfPSrwaerWPJEzoX5xUVRWUmQDgVJkEqz\"]}}",
        hash, args
    );
    println!("{}", all_args);
    root.call(
        factory.account_id.clone(),
        "create",
        all_args.as_bytes(),
        near_sdk_sim::DEFAULT_GAS,
        to_yocto("10"),
    )
    .assert_success();
    let acc = root.borrow_runtime().view_account("test.xyz").unwrap();
    assert_eq!(acc.code_hash.to_string(), hash);
    // due to contract rewards.
    assert!(acc.amount > to_yocto("10"));
}
