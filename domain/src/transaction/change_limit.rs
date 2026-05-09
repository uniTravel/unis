use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct ChangeLimit {
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    #[schema(minimum = 10000, maximum = 10000000)]
    /// 账户限额
    pub limit: i64,
}

impl Command for ChangeLimit {
    type A = super::Transaction;
    type E = LimitChanged;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("待修改限额须大于零".to_string()));
        }
        if self.limit == agg.limit {
            return Err(UniError::CheckError("限额无变化".to_string()));
        }

        Ok(())
    }

    fn apply(self, agg: &Self::A) -> Self::E {
        Self::E {
            limit: self.limit,
            trans_limit: if agg.trans_limit > self.limit {
                self.limit
            } else {
                agg.trans_limit
            },
        }
    }
}

#[event]
pub struct LimitChanged {
    limit: i64,
    trans_limit: i64,
}

impl Event for LimitChanged {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
        agg.trans_limit = self.trans_limit;
    }
}

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn change_limit() (
        limit in 10_000..10_000_000i64
    ) -> ChangeLimit {
        ChangeLimit { limit }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        fn invalid_limit_range() (
            limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX]
        ) -> ChangeLimit {
            ChangeLimit { limit }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in change_limit()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn limit_range(com in invalid_limit_range()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("值应介于 10000 到 10000000 之间"));
        }
    }
}
