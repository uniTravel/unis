pub mod account;
pub mod transaction;

use unis::macros::aggregate;

#[aggregate]
pub struct Account {
    code: String,
    owner: String,
    limit: i64,
    verified_by: String,
    verified: bool,
    verify_conclusion: bool,
    approved_by: String,
    approved: bool,
}

#[aggregate]
pub struct Transaction {
    account_code: String,
    balance: i64,
    period: String,
    limit: i64,
    trans_limit: i64,
}
