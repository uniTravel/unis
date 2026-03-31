use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct VerifyAccount {
    #[validate(length(min = 1))]
    pub verified_by: String,
    pub verified: bool,
}

impl Command for VerifyAccount {
    type A = super::Account;
    type E = AccountVerified;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.code.is_empty() {
            return Err(UniError::CheckError("尚未创建，不能审核".to_string()));
        }
        if !agg.verified_by.is_empty() {
            return Err(UniError::CheckError(format!(
                "已经审核，结论为 {}",
                agg.verified
            )));
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

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        pub fn valid_com() (
            verified_by in long_string(1),
            verified in prop::bool::ANY
        ) -> VerifyAccount {
            VerifyAccount { verified_by, verified }
        }
    }

    proptest! {
        #![proptest_config(proptest_config())]

        #[test]
        fn valid_command(com in valid_com()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_ok());
        }
    }
}
