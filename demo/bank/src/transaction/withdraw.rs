use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct Withdraw {
    amount: i64,
}

impl Command for Withdraw {
    type A = super::Transaction;
    type E = WithdrawFinished;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if self.amount > agg.balance {
            return Err(UniError::CheckError("余额不足".to_string()));
        }
        if self.amount > agg.trans_limit {
            return Err(UniError::CheckError("金额超限".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            amount: self.amount,
            balance: agg.balance - self.amount,
        }
    }
}

#[event]
pub struct WithdrawFinished {
    account_code: String,
    amount: i64,
    balance: i64,
}

impl Event for WithdrawFinished {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}
