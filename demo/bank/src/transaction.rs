use unis::{
    BINCODE_CONFIG,
    domain::{Command, Event, Load},
    errors::UniError,
    macros::{aggregate, command_enum, event_enum},
};
use uuid::Uuid;

mod change_limit;
mod deposit;
mod init;
mod open;
mod set_limit;
mod set_trans_limit;
mod transfer_in;
mod transfer_out;
mod withdraw;

pub use change_limit::ChangeLimit;
pub use deposit::Deposit;
pub use init::InitPeriod;
pub use open::OpenPeriod;
pub use set_limit::SetLimit;
pub use set_trans_limit::SetTransLimit;
pub use transfer_in::TransferIn;
pub use transfer_out::TransferOut;
pub use withdraw::Withdraw;

#[aggregate]
pub struct Transaction {
    account_code: String,
    balance: i64,
    period: String,
    limit: i64,
    trans_limit: i64,
}

#[event_enum(Transaction)]
pub enum TransactionEvent {
    PeriodInited(init::PeriodInited) = 0,
    PeriodOpened(open::PeriodOpened) = 1,
    LimitSetted(set_limit::LimitSetted) = 2,
    LimitChanged(change_limit::LimitChanged) = 3,
    TransLimitSetted(set_trans_limit::TransLimitSetted) = 4,
    DepositFinished(deposit::DepositFinished) = 5,
    WithdrawFinished(withdraw::WithdrawFinished) = 6,
    TransferOutFinished(transfer_out::TransferOutFinished) = 7,
    TransferInFinished(transfer_in::TransferInFinished) = 8,
}

#[command_enum(Transaction)]
pub enum TransactionCommand {
    InitPeriod(init::InitPeriod) = 0,
    OpenPeriod(open::OpenPeriod) = 1,
    SetLimit(set_limit::SetLimit) = 2,
    ChangeLimit(change_limit::ChangeLimit) = 3,
    SetTransLimit(set_trans_limit::SetTransLimit) = 4,
    Deposit(deposit::Deposit) = 5,
    Withdraw(withdraw::Withdraw) = 6,
    TransferOut(transfer_out::TransferOut) = 7,
    TransferIn(transfer_in::TransferIn) = 8,
}

pub async fn dispatcher(
    agg_type: &'static str,
    agg_id: Uuid,
    com_data: Vec<u8>,
    mut agg: Transaction,
    loader: impl Load,
) -> Result<(Transaction, TransactionEvent), UniError> {
    let (com, _): (TransactionCommand, _) = bincode::decode_from_slice(&com_data, BINCODE_CONFIG)?;
    match com {
        TransactionCommand::InitPeriod(com) => {
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::PeriodInited(evt)))
        }
        TransactionCommand::OpenPeriod(com) => {
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::PeriodOpened(evt)))
        }
        TransactionCommand::SetLimit(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::LimitSetted(evt)))
        }
        TransactionCommand::ChangeLimit(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::LimitChanged(evt)))
        }
        TransactionCommand::SetTransLimit(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::TransLimitSetted(evt)))
        }
        TransactionCommand::Deposit(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::DepositFinished(evt)))
        }
        TransactionCommand::Withdraw(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::WithdrawFinished(evt)))
        }
        TransactionCommand::TransferOut(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::TransferOutFinished(evt)))
        }
        TransactionCommand::TransferIn(com) => {
            replay(agg_type, agg_id, &mut agg, loader).await?;
            let evt = com.process(&mut agg)?;
            Ok((agg, TransactionEvent::TransferInFinished(evt)))
        }
    }
}
