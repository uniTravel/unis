use unis::{
    domain::{Command, Event},
    errors::CheckError,
    macros::{command, event},
};

#[command]
pub struct VerifyAccount {
    #[validate(length(min = 1))]
    #[schema(min_length = 1)]
    /// 审核人
    pub verified_by: String,
    /// 审核结论
    pub verified: bool,
}

impl Command for VerifyAccount {
    type A = super::Account;
    type E = AccountVerified;

    fn check(&self, agg: &Self::A) -> Result<(), CheckError> {
        if agg.code.is_empty() {
            return Err(CheckError("尚未创建，不能审核"));
        }
        if !agg.verified_by.is_empty() {
            return Err(CheckError("已经审核"));
        }

        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            verified_by: self.verified_by,
            verified: self.verified,
        }
    }
}

#[event]
pub struct AccountVerified {
    verified_by: String,
    verified: bool,
}

impl Event for AccountVerified {
    type A = super::Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.verified_by = self.verified_by.clone();
        agg.verified = self.verified;
    }
}

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn verify() (
        verified_by in crate::tests::long_string(1),
        verified in proptest::bool::ANY
    ) -> VerifyAccount {
        VerifyAccount { verified_by, verified }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn valid_command(com in verify()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_ok());
        }
    }
}
