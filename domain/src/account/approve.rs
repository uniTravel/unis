use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct ApproveAccount {
    #[validate(length(min = 1))]
    pub approved_by: String,
    pub approved: bool,
    #[validate(range(min = 10_000, max = 10_000_000, code = "range_num"))]
    pub limit: i64,
}

impl Command for ApproveAccount {
    type A = super::Account;
    type E = AccountApproved;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if !agg.verified {
            return Err(UniError::CheckError("审核未通过".to_string()));
        }
        if !agg.approved_by.is_empty() {
            return Err(UniError::CheckError(format!(
                "已经审批，结论为 {}",
                agg.approved
            )));
        }

        Ok(())
    }

    fn apply(self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.code.clone(),
            approved_by: self.approved_by,
            approved: self.approved,
            limit: self.limit,
        }
    }
}

#[event]
pub struct AccountApproved {
    account_code: String,
    approved_by: String,
    approved: bool,
    limit: i64,
}

impl Event for AccountApproved {
    type A = super::Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.approved_by = self.approved_by.clone();
        agg.approved = self.approved;
        agg.limit = self.limit;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        pub fn valid_com() (
            approved_by in long_string(1),
            approved in prop::bool::ANY,
            limit in 10_000..10_000_000i64
        ) -> ApproveAccount {
            ApproveAccount { approved_by, approved, limit }
        }
    }

    prop_compose! {
        fn invalid_limit_range() (
            approved_by in long_string(1),
            approved in prop::bool::ANY,
            limit in prop_oneof![..10_000i64, 10_000_000..i64::MAX]
        ) -> ApproveAccount {
            ApproveAccount { approved_by, approved, limit }
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
