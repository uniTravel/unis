use crate::*;
use domain::transaction::OpenPeriod;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
        account_code in digit_string(6)
    ) -> OpenPeriod {
        OpenPeriod { account_code }
    }
}

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
}
