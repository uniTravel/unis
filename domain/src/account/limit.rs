use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct LimitAccount {
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    pub limit: i64,
}

impl Command for LimitAccount {
    type A = super::Account;
    type E = AccountLimited;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if !agg.approved {
            return Err(UniError::CheckError("账户未批准".to_string()));
        }
        if self.limit == agg.limit {
            return Err(UniError::CheckError("限额无变化".to_string()));
        }

        Ok(())
    }

    fn apply(self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.code.clone(),
            limit: self.limit,
        }
    }
}

#[event]
pub struct AccountLimited {
    account_code: String,
    limit: i64,
}

impl Event for AccountLimited {
    type A = super::Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        pub fn valid_com() (
            limit in 10_000..10_000_000i64
        ) -> LimitAccount {
            LimitAccount { limit }
        }
    }

    prop_compose! {
        fn invalid_limit_range() (
            limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX]
        ) -> LimitAccount {
            LimitAccount { limit }
        }
    }

    proptest! {
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
    }
}
