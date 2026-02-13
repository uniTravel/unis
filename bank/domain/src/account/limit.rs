use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct LimitAccount {
    #[validate(range(min = 10000, max = 10000000))]
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
