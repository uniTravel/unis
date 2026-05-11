use crate::validate;
use chrono::Local;
use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

/// * 账户设立时执行一次，后续都是打开交易期。
#[command]
pub struct InitPeriod {
    #[validate(
        length(equal = 6, code = "exact_length"),
        custom(function = "validate::code")
    )]
    #[schema(pattern = r"^\d{6}$")]
    /// 账号
    pub account_code: String,
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    #[schema(minimum = 10000, maximum = 10000000)]
    /// 账户限额
    pub limit: i64,
}

impl Command for InitPeriod {
    type A = super::Transaction;
    type E = PeriodInited;

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            account_code: self.account_code,
            period: Local::now().format("%Y-%m").to_string(),
            limit: self.limit,
        }
    }
}

#[event]
pub struct PeriodInited {
    account_code: String,
    period: String,
    limit: i64,
}

impl Event for PeriodInited {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.account_code = self.account_code.clone();
        agg.period = self.period.clone();
        agg.limit = self.limit;
        agg.trans_limit = self.limit;
    }
}

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn init() (
        account_code in crate::tests::digit_string(6),
        limit in 10_000..10_000_000i64
    ) -> InitPeriod {
        InitPeriod { account_code, limit }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        fn invalid_code_length() (
            account_code in prop_oneof![short_string(5), long_string(7)],
            limit in 10_000..10_000_000i64
        ) -> InitPeriod {
            InitPeriod { account_code, limit }
        }
    }

    prop_compose! {
        fn invalid_code_type() (
            account_code in prop::string::string_regex("[^0-9]{6}").unwrap(),
            limit in 10_000..10_000_000i64
        ) -> InitPeriod {
            InitPeriod { account_code, limit }
        }
    }

    prop_compose! {
        fn invalid_limit_range() (
            account_code in digit_string(6),
            limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX]
        ) -> InitPeriod {
            InitPeriod { account_code, limit }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in init()) {
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
        fn limit_range(com in invalid_limit_range()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("值应介于 10000 到 10000000 之间"));
        }
    }
}
