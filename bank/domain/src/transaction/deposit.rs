use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct Deposit {
    amount: i64,
}

impl Command for Deposit {
    type A = super::Transaction;
    type E = DepositFinished;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if self.amount > agg.trans_limit {
            return Err(UniError::CheckError("金额超限".to_string()));
        }

        Ok(())
    }

    fn apply(self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            amount: self.amount,
            balance: agg.balance + self.amount,
        }
    }
}

#[event]
pub struct DepositFinished {
    account_code: String,
    amount: i64,
    balance: i64,
}

impl Event for DepositFinished {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}
