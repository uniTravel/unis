use crate::*;
use domain::transaction::TransferIn;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
        out_code in digit_string(6),
        amount in 1..i64::MAX
    ) -> TransferIn {
        TransferIn { out_code, amount }
    }
}

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
    fn amount_range(com in invalid_amount_range()) {
        let result = unis::validate(&com, "zh");
        prop_assert!(result.is_err());
        prop_assert!(result.unwrap_err().contains("最小值为 1"))
    }
}
