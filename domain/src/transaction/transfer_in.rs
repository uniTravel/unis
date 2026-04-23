use crate::validate;
use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct TransferIn {
    #[validate(
        length(equal = 6, code = "exact_length"),
        custom(function = "validate::code")
    )]
    pub out_code: String,
    #[validate(range(min = 1, code = "min_num"))]
    pub amount: i64,
}

impl Command for TransferIn {
    type A = super::Transaction;
    type E = TransferInFinished;

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
            out_code: self.out_code,
            amount: self.amount,
            balance: agg.balance + self.amount,
        }
    }
}

#[event]
pub struct TransferInFinished {
    account_code: String,
    out_code: String,
    amount: i64,
    balance: i64,
}

impl Event for TransferInFinished {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn transfer_in() (
        out_code in crate::tests::digit_string(6),
        amount in 1..i64::MAX
    ) -> TransferIn {
        TransferIn { out_code, amount }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        fn invalid_code_length() (
            out_code in prop_oneof![short_string(5), long_string(7)],
            amount in 1..i64::MAX
        ) -> TransferIn {
            TransferIn { out_code, amount }
        }
    }

    prop_compose! {
        fn invalid_code_type() (
            out_code in prop::string::string_regex("[^0-9]{6}").unwrap(),
            amount in 1..i64::MAX
        ) -> TransferIn {
            TransferIn { out_code, amount }
        }
    }

    prop_compose! {
        fn invalid_amount_range() (
            out_code in digit_string(6),
            amount in ..1i64
        ) -> TransferIn {
            TransferIn { out_code, amount }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in transfer_in()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn code_length(com in invalid_code_length()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("长度应为 6"));
        }

        #[test]
        fn code_type(com in invalid_code_type()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("应为 ASCII 数字"));
        }

        #[test]
        fn amount_range(com in invalid_amount_range()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("最小值为 1"))
        }
    }
}
