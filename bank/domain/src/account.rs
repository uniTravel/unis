use unis::{
    domain::{Command, CommandEnum, Event, Load},
    errors::UniError,
    macros::{aggregate, command_enum, event_enum},
};
use uuid::Uuid;

mod approve;
mod create;
mod limit;
mod verify;

pub use approve::ApproveAccount;
pub use create::CreateAccount;
pub use limit::LimitAccount;
pub use verify::VerifyAccount;

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

#[event_enum(Account)]
pub enum AccountEvent {
    Created(create::AccountCreated) = 0,
    Verified(verify::AccountVerified) = 1,
    Limited(limit::AccountLimited) = 2,
    Approved(approve::AccountApproved) = 3,
}

#[command_enum]
pub enum AccountCommand {
    Create(create::CreateAccount) = 0,
    Verify(verify::VerifyAccount) = 1,
    Limit(limit::LimitAccount) = 2,
    Approve(approve::ApproveAccount) = 3,
}

impl CommandEnum for AccountCommand {
    type A = Account;
    type E = AccountEvent;

    async fn apply(
        self,
        agg_type: &'static str,
        agg_id: Uuid,
        mut agg: Self::A,
        loader: impl Load<Self::E>,
    ) -> Result<(Self::A, Self::E), UniError> {
        match self {
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
}
