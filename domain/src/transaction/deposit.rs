use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct Deposit {
    #[validate(range(min = 1, code = "min_num"))]
    pub amount: i64,
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

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        pub fn valid_com() (
            amount in 1..i64::MAX
        ) -> Deposit {
            Deposit { amount }
        }
    }

    prop_compose! {
        fn invalid_amount_range() (
            amount in ..1i64
        ) -> Deposit {
            Deposit { amount }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in valid_com()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn amount_range(com in invalid_amount_range()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("最小值为 1"))
        }
    }
}
