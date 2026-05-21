#[cfg(feature = "test-utils")]
pub mod tests;
mod validate;

use unis::macros::aggregate;

pub mod account;
#[aggregate]
pub struct Account {
    pub code: String,
    pub owner: String,
    pub limit: i64,
    pub verified_by: String,
    pub verified: bool,
    pub approved_by: String,
    pub approved: bool,
}

pub mod transaction;
#[aggregate]
pub struct Transaction {
    pub account_code: String,
    pub balance: i64,
    pub period: String,
    pub limit: i64,
    pub trans_limit: i64,
}
