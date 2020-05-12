# Basic Mutlisig contract

*This is an experimental contract. Please use only on TestNet.*

This contract provides:
 - Set K out of N multi sig scheme
 - Request to sign a transfer or function call
 - Any of the access keys can confirm, until the required number is matched.

## Pre-requisites

To develop Rust contracts you would need to:
* Install [Rustup](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
* Add wasm target to your toolchain:
```bash
rustup target add wasm32-unknown-unknown
```

## Building the contract

```bash
./build.sh
```

## Usage

Commands to deploy and initialize a 2 out of 3 multisig contract via `near repl`:

```javascript
const tx = nearlib.transaction.createTransaction(
[
    nearlib.transactions.createAccount(),
    nearlib.transactions.transfer(10),  
    nearlib.transactions.addKey(`<1st_public_key>`),
    nearlib.transactions.addKey(`<2nd_public_key>`),
    nearlib.transactions.addKey(`<3nd_public_key>`),
    nearlib.transactions.deploy(),
    nearlib.transactions.functionCall("new", {"num_confirmations": 2}, 0, 10000000000000000),
]);
```
