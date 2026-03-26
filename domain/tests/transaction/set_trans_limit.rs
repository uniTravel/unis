use crate::*;
use domain::transaction::SetTransLimit;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
        trans_limit in 10_000..10_000_000i64
    ) -> SetTransLimit {
        SetTransLimit { trans_limit }
    }
}

prop_compose! {
    fn invalid_trans_limit_range() (
        trans_limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX]
    ) -> SetTransLimit {
        SetTransLimit { trans_limit }
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
    fn trans_limit_range(com in invalid_trans_limit_range()) {
        let result = unis::validate(&com, "zh");
        prop_assert!(result.is_err());
        prop_assert!(result.unwrap_err().contains("值应介于 10000 到 10000000 之间"));
    }
}
