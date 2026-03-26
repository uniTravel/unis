use crate::*;
use domain::transaction::InitPeriod;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
        account_code in digit_string(6),
        limit in 10_000..10_000_000i64
    ) -> InitPeriod {
        InitPeriod { account_code, limit }
    }
}

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
    #![proptest_config(proptest_config())]

    #[test]
    fn valid_command(com in valid_com()) {
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
