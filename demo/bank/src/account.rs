use unis::{
    BINCODE_CONFIG,
    domain::{Command, Event, Load},
    errors::UniError,
    macros::*,
};
use uuid::Uuid;

#[aggregate]
pub struct Account {
    code: String,
    owner: String,
    limit: i64,
    verified_by: String,
    verified: bool,
    verify_conclusion: bool,
    approved_by: String,
    approved: bool,
}

#[command]
pub struct CreateAccount {
    #[validate(length(min = 6, max = 6))]
    code: String,
    owner: String,
}

impl Command for CreateAccount {
    type A = Account;
    type E = AccountCreated;

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn execute(&self, _agg: &Self::A) -> Self::E {
        Self::E {
            code: self.code.clone(),
            owner: self.owner.clone(),
        }
    }
}

#[event]
pub struct AccountCreated {
    code: String,
    owner: String,
}

impl Event for AccountCreated {
    type A = Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.code = self.code.clone();
        agg.owner = self.owner.clone();
    }
}

#[command]
pub struct VerifyAccount {
    verified_by: String,
    conclusion: bool,
}

impl Command for VerifyAccount {
    type A = Account;
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
    type A = Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.verified_by = self.verified_by.clone();
        agg.verified = self.verified;
        agg.verify_conclusion = self.conclusion;
    }
}

#[command]
pub struct LimitAccount {
    #[validate(range(min = 10000, max = 10000000))]
    limit: i64,
}

impl Command for LimitAccount {
    type A = Account;
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

    fn execute(&self, agg: &Self::A) -> Self::E {
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
    type A = Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
    }
}

#[command]
pub struct ApproveAccount {
    approved_by: String,
    approved: bool,
    #[validate(range(min = 10000, max = 10000000))]
    limit: i64,
}

impl Command for ApproveAccount {
    type A = Account;
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
    type A = Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.approved_by = self.approved_by.clone();
        agg.approved = self.approved;
        agg.limit = self.limit;
    }
}

#[event_enum(Account)]
pub enum AccountEvent {
    Created(AccountCreated) = 0,
    Verified(AccountVerified) = 1,
    Limited(AccountLimited) = 2,
    Approved(AccountApproved) = 3,
}

#[command_enum(Account)]
pub enum AccountCommand {
    Create(CreateAccount) = 0,
    Verify(VerifyAccount) = 1,
    Limit(LimitAccount) = 2,
    Approve(ApproveAccount) = 3,
}

pub async fn dispatcher(
    agg_type: &'static str,
    agg_id: Uuid,
    com_data: Vec<u8>,
    mut agg: Account,
    loader: impl Load,
) -> Result<(Account, AccountEvent), UniError> {
    let (com, _): (AccountCommand, _) = bincode::decode_from_slice(&com_data, BINCODE_CONFIG)?;
    match com {
        AccountCommand::Create(com) => {
            let evt = com.process(&mut agg)?;
            Ok((agg, AccountEvent::Created(evt)))
        }
        AccountCommand::Verify(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, AccountEvent::Verified(evt)))
        }
        AccountCommand::Limit(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, AccountEvent::Limited(evt)))
        }
        AccountCommand::Approve(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, AccountEvent::Approved(evt)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn create_account() {
        let com = CreateAccount {
            code: "1234".to_string(),
            owner: "张三".to_string(),
        };
        if let Err(e) = com.validate() {
            println!("{e}");
        }
    }
}
