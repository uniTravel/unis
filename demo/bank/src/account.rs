use unis::{
    BINCODE_CONFIG,
    domain::{Command, Event, Load},
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

#[command_enum(Account)]
pub enum AccountCommand {
    Create(create::CreateAccount) = 0,
    Verify(verify::VerifyAccount) = 1,
    Limit(limit::LimitAccount) = 2,
    Approve(approve::ApproveAccount) = 3,
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
