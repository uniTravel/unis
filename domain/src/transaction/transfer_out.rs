use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct TransferOut {
    pub in_code: String,
    pub amount: i64,
}

impl Command for TransferOut {
    type A = super::Transaction;
    type E = TransferOutFinished;

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

    fn apply(self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            in_code: self.in_code,
            amount: self.amount,
            balance: agg.balance - self.amount,
        }
    }
}

#[event]
pub struct TransferOutFinished {
    account_code: String,
    in_code: String,
    amount: i64,
    balance: i64,
}

impl Event for TransferOutFinished {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}
