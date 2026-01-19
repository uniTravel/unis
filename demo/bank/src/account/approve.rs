use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct ApproveAccount {
    approved_by: String,
    approved: bool,
    #[validate(range(min = 10000, max = 10000000))]
    limit: i64,
}

impl Command for ApproveAccount {
    type A = super::Account;
    type E = AccountApproved;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if !agg.verify_conclusion {
            return Err(UniError::CheckError("审核未通过".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.code.clone(),
            approved_by: self.approved_by.clone(),
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
