use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct SetTransLimit {
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    pub trans_limit: i64,
}

impl Command for SetTransLimit {
    type A = super::Transaction;
    type E = TransLimitSetted;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if self.trans_limit > agg.limit {
            return Err(UniError::CheckError("交易限额不得超过控制限额".to_string()));
        }
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            trans_limit: self.trans_limit,
        }
    }
}

#[event]
pub struct TransLimitSetted {
    trans_limit: i64,
}

impl Event for TransLimitSetted {
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.trans_limit = self.trans_limit;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        pub fn valid_com() (
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
}
