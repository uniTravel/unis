use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct VerifyAccount {
    verified_by: String,
    conclusion: bool,
}

impl Command for VerifyAccount {
    type A = super::Account;
    type E = AccountVerified;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.verified {
            return Err(UniError::CheckError(format!(
                "已经审核，结论为 {}",
                agg.verify_conclusion
            )));
        }

        Ok(())
    }

    fn execute(&self, _agg: &Self::A) -> Self::E {
        Self::E {
            verified_by: self.verified_by.clone(),
            verified: true,
            conclusion: self.conclusion,
        }
    }
}

#[event]
pub struct AccountVerified {
    verified_by: String,
    verified: bool,
    conclusion: bool,
}

impl Event for AccountVerified {
    type A = super::Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.verified_by = self.verified_by.clone();
        agg.verified = self.verified;
        agg.verify_conclusion = self.conclusion;
    }
}
