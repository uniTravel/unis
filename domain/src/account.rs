#[cfg(test)]
mod tests;

mod approve;
mod create;
mod limit;
mod verify;

pub use approve::ApproveAccount;
pub use create::CreateAccount;
pub use limit::LimitAccount;
pub use verify::VerifyAccount;

#[cfg(feature = "test-utils")]
pub use approve::approve;
#[cfg(feature = "test-utils")]
pub use create::create;
#[cfg(feature = "test-utils")]
pub use limit::limit;
#[cfg(feature = "test-utils")]
pub use verify::verify;

use unis::{
    domain::{Command, CommandEnum, Event, Load},
    errors::UniError,
    macros::{aggregate, command_enum, event_enum},
};
use uuid::Uuid;

#[aggregate]
pub struct Account {
    pub code: String,
    pub owner: String,
    pub limit: i64,
    pub verified_by: String,
    pub verified: bool,
    pub approved_by: String,
    pub approved: bool,
}

#[event_enum(Account)]
pub enum AccountEvent {
    Created(create::AccountCreated) = 0,
    Verified(verify::AccountVerified) = 1,
    Approved(approve::AccountApproved) = 2,
    Limited(limit::AccountLimited) = 3,
}

#[command_enum]
pub enum AccountCommand {
    Create(create::CreateAccount) = 0,
    Verify(verify::VerifyAccount) = 1,
    Approve(approve::ApproveAccount) = 2,
    Limit(limit::LimitAccount) = 3,
}

impl CommandEnum for AccountCommand {
    type A = Account;
    type E = AccountEvent;

    async fn apply(
        self,
        topic: &'static str,
        agg_id: Uuid,
        mut agg: Self::A,
        coms: &mut ahash::AHashSet<Uuid>,
        loader: impl Load<Self::E>,
    ) -> Result<(Self::A, Self::E), UniError> {
        match self {
            AccountCommand::Create(com) => {
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Created(evt)))
            }
            AccountCommand::Verify(com) => {
                replay(topic, agg_id, &mut agg, coms, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Verified(evt)))
            }
            AccountCommand::Limit(com) => {
                replay(topic, agg_id, &mut agg, coms, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Limited(evt)))
            }
            AccountCommand::Approve(com) => {
                replay(topic, agg_id, &mut agg, coms, loader).await?;
                let evt = com.process(&mut agg)?;
                Ok((agg, AccountEvent::Approved(evt)))
            }
        }
    }
}
