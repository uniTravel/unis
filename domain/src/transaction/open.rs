use crate::validate;
use chrono::Local;
use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct OpenPeriod {
    #[validate(
        length(equal = 6, code = "exact_length"),
        custom(function = "validate::code")
    )]
    #[schema(pattern = r"^\d{6}$")]
    /// 账号
    pub account_code: String,
}

impl Command for OpenPeriod {
    type A = super::Transaction;
    type E = PeriodOpened;

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            account_code: self.account_code,
            period: Local::now().format("%Y-%m").to_string(),
        }
    }
}

#[event]
pub struct PeriodOpened {
    account_code: String,
    period: String,
}

impl Event for PeriodOpened {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.account_code = self.account_code.clone();
        agg.period = self.period.clone();
    }
}

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn open() (
        account_code in crate::tests::digit_string(6)
    ) -> OpenPeriod {
        OpenPeriod { account_code }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        fn invalid_code_length() (
            account_code in prop_oneof![short_string(5), long_string(7)]
        ) -> OpenPeriod {
            OpenPeriod { account_code }
        }
    }

    prop_compose! {
        fn invalid_code_type() (
            account_code in prop::string::string_regex("[^0-9]{6}").unwrap()
        ) -> OpenPeriod {
            OpenPeriod { account_code }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in open()) {
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
    }
}
