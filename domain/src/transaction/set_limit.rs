use crate::validate;
use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
#[validate(schema(function = "validate::validate_set_limit"))]
pub struct SetLimit {
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    pub limit: i64,
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    pub trans_limit: i64,
    #[validate(range(min = 0, code = "min_num"))]
    pub balance: i64,
}

impl Command for SetLimit {
    type A = super::Transaction;
    type E = LimitSetted;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.account_code.is_empty() {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if agg.limit != 0 {
            return Err(UniError::CheckError("不得重复设置限额".to_string()));
        }

        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            limit: self.limit,
            trans_limit: self.trans_limit,
            balance: self.balance,
        }
    }
}

#[event]
pub struct LimitSetted {
    limit: i64,
    trans_limit: i64,
    balance: i64,
}

impl Event for LimitSetted {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
        agg.trans_limit = self.trans_limit;
        agg.balance = self.balance;
    }
}

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn set_limit() (
        (l, g) in crate::tests::less_great_equal(10_000..10_000_000i64, 10_000..10_000_000i64),
        balance in 0..i64::MAX
    ) -> SetLimit {
        SetLimit { limit: g, trans_limit: l, balance }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

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

    prop_compose! {
        pub fn invalid_trans_limit() (
            (l, g) in less_great(10_000..10_000_000i64, 10_000..10_000_000i64),
            balance in 0..i64::MAX
        ) -> SetLimit {
            SetLimit { limit: l, trans_limit: g, balance }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in set_limit()) {
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

        #[test]
        fn trans_limit(com in invalid_trans_limit()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("交易限额不得大于控制限额"))
        }
    }
}
