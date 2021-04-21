use crate::*;

#[near_bindgen]
impl Contract {
    pub fn storage_minimum_balance(&self) -> U128 {
        self.ft.storage_balance_bounds().min
    }
}
