use crate::*;
use domain::transaction::SetLimit;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
        limit in 10_000..10_000_000i64,
        trans_limit in 10_000..10_000_000i64,
        balance in 0..i64::MAX
    ) -> SetLimit {
        SetLimit { limit, trans_limit, balance }
    }
}

prop_compose! {
    fn invalid_limit_range() (
        limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX],
        trans_limit in 10_000..10_000_000i64,
        balance in 0..i64::MAX
    ) -> SetLimit {
        SetLimit { limit, trans_limit, balance }
    }
}

prop_compose! {
    fn invalid_trans_limit_range() (
        limit in 10_000..10_000_000i64,
        trans_limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX],
        balance in 0..i64::MAX
    ) -> SetLimit {
        SetLimit { limit, trans_limit, balance }
    }
}

prop_compose! {
    fn invalid_balance_range() (
        limit in 10_000..10_000_000i64,
        trans_limit in 10_000..10_000_000i64,
        balance in ..0i64
    ) -> SetLimit {
        SetLimit { limit, trans_limit, balance }
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
    fn limit_range(com in invalid_limit_range()) {
        let result = unis::validate(&com, "zh");
        prop_assert!(result.is_err());
        prop_assert!(result.unwrap_err().contains("值应介于 10000 到 10000000 之间"));
    }

    #[test]
    fn trans_limit_range(com in invalid_trans_limit_range()) {
        let result = unis::validate(&com, "zh");
        prop_assert!(result.is_err());
        prop_assert!(result.unwrap_err().contains("值应介于 10000 到 10000000 之间"));
    }

    #[test]
    fn balance_range(com in invalid_balance_range()) {
        let result = unis::validate(&com, "zh");
        prop_assert!(result.is_err());
        prop_assert!(result.unwrap_err().contains("最小值为 0"))
    }
}
