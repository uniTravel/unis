use crate::*;
use domain::transaction::Deposit;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
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
    #![proptest_config(proptest_config())]

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
