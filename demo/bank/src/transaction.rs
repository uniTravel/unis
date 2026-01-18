use unis::{
    BINCODE_CONFIG,
    domain::{Command, Event, Load},
    errors::UniError,
    macros::*,
};
use uuid::Uuid;

#[aggregate]
pub struct Transaction {
    account_code: String,
    balance: i64,
    period: String,
    limit: i64,
    trans_limit: i64,
}

#[command]
pub struct InitPeriod {
    account_code: String,
    limit: i64,
}

impl Command for InitPeriod {
    type A = Transaction;
    type E = PeriodInited;

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn execute(&self, _agg: &Self::A) -> Self::E {
        Self::E {
            account_code: self.account_code.clone(),
            period: "".to_string(),
            limit: self.limit,
        }
    }
}

#[event]
pub struct PeriodInited {
    account_code: String,
    period: String,
    limit: i64,
}

impl Event for PeriodInited {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.account_code = self.account_code.clone();
        agg.period = self.period.clone();
        agg.limit = self.limit;
    }
}

#[command]
pub struct OpenPeriod {
    account_code: String,
    period: String,
}

impl Command for OpenPeriod {
    type A = Transaction;
    type E = PeriodOpened;

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn execute(&self, _agg: &Self::A) -> Self::E {
        Self::E {
            account_code: self.account_code.clone(),
            period: "".to_string(),
        }
    }
}

#[event]
pub struct PeriodOpened {
    account_code: String,
    period: String,
}

impl Event for PeriodOpened {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.account_code = self.account_code.clone();
        agg.period = self.period.clone();
    }
}

#[command]
pub struct SetLimit {
    limit: i64,
    trans_limit: i64,
    balance: i64,
}

impl Command for SetLimit {
    type A = Transaction;
    type E = LimitSetted;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit != 0 {
            return Err(UniError::CheckError("不得重复设置限额".to_string()));
        }

        Ok(())
    }

    fn execute(&self, _agg: &Self::A) -> Self::E {
        Self::E {
            limit: self.limit,
            trans_limit: self.trans_limit,
            balance: self.balance,
        }
    }
}

#[event]
pub struct LimitSetted {
    limit: i64,
    trans_limit: i64,
    balance: i64,
}

impl Event for LimitSetted {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
        agg.trans_limit = self.trans_limit;
        agg.balance = self.balance;
    }
}

#[command]
pub struct ChangeLimit {
    limit: i64,
}

impl Command for ChangeLimit {
    type A = Transaction;
    type E = LimitChanged;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("待修改限额须大于零".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            limit: self.limit,
            trans_limit: if agg.trans_limit > self.limit {
                self.limit
            } else {
                agg.trans_limit
            },
        }
    }
}

#[event]
pub struct LimitChanged {
    limit: i64,
    trans_limit: i64,
}

impl Event for LimitChanged {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
        agg.trans_limit = self.trans_limit;
    }
}

#[command]
pub struct SetTransLimit {
    trans_limit: i64,
}

impl Command for SetTransLimit {
    type A = Transaction;
    type E = TransLimitSetted;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if self.trans_limit > agg.limit {
            return Err(UniError::CheckError("交易限额不得超过控制限额".to_string()));
        }
        Ok(())
    }

    fn execute(&self, _agg: &Self::A) -> Self::E {
        Self::E {
            trans_limit: self.trans_limit,
        }
    }
}

#[event]
pub struct TransLimitSetted {
    trans_limit: i64,
}

impl Event for TransLimitSetted {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.trans_limit = self.trans_limit;
    }
}

#[command]
pub struct Deposit {
    amount: i64,
}

impl Command for Deposit {
    type A = Transaction;
    type E = DepositFinished;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if self.amount > agg.trans_limit {
            return Err(UniError::CheckError("金额超限".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            amount: self.amount,
            balance: agg.balance + self.amount,
        }
    }
}

#[event]
pub struct DepositFinished {
    account_code: String,
    amount: i64,
    balance: i64,
}

impl Event for DepositFinished {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}

#[command]
pub struct Withdraw {
    amount: i64,
}

impl Command for Withdraw {
    type A = Transaction;
    type E = WithdrawFinished;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if self.amount > agg.balance {
            return Err(UniError::CheckError("余额不足".to_string()));
        }
        if self.amount > agg.trans_limit {
            return Err(UniError::CheckError("金额超限".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            amount: self.amount,
            balance: agg.balance - self.amount,
        }
    }
}

#[event]
pub struct WithdrawFinished {
    account_code: String,
    amount: i64,
    balance: i64,
}

impl Event for WithdrawFinished {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}

#[command]
pub struct TransferOut {
    in_code: String,
    amount: i64,
}

impl Command for TransferOut {
    type A = Transaction;
    type E = TransferOutFinished;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if self.amount > agg.balance {
            return Err(UniError::CheckError("余额不足".to_string()));
        }
        if self.amount > agg.trans_limit {
            return Err(UniError::CheckError("金额超限".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            in_code: self.in_code.clone(),
            amount: self.amount,
            balance: agg.balance - self.amount,
        }
    }
}

#[event]
pub struct TransferOutFinished {
    account_code: String,
    in_code: String,
    amount: i64,
    balance: i64,
}

impl Event for TransferOutFinished {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}

#[command]
pub struct TransferIn {
    out_code: String,
    amount: i64,
}

impl Command for TransferIn {
    type A = Transaction;
    type E = TransferInFinished;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("交易期间尚未生效".to_string()));
        }
        if self.amount > agg.trans_limit {
            return Err(UniError::CheckError("金额超限".to_string()));
        }

        Ok(())
    }

    fn execute(&self, agg: &Self::A) -> Self::E {
        Self::E {
            account_code: agg.account_code.clone(),
            out_code: self.out_code.clone(),
            amount: self.amount,
            balance: agg.balance + self.amount,
        }
    }
}

#[event]
pub struct TransferInFinished {
    account_code: String,
    out_code: String,
    amount: i64,
    balance: i64,
}

impl Event for TransferInFinished {
    type A = Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.balance = self.balance;
    }
}

#[event_enum(Transaction)]
pub enum TransactionEvent {
    PeriodInited(PeriodInited) = 0,
    PeriodOpened(PeriodOpened) = 1,
    LimitSetted(LimitSetted) = 2,
    LimitChanged(LimitChanged) = 3,
    TransLimitSetted(TransLimitSetted) = 4,
    DepositFinished(DepositFinished) = 5,
    WithdrawFinished(WithdrawFinished) = 6,
    TransferOutFinished(TransferOutFinished) = 7,
    TransferInFinished(TransferInFinished) = 8,
}

#[command_enum(Transaction)]
pub enum TransactionCommand {
    InitPeriod(InitPeriod) = 0,
    OpenPeriod(OpenPeriod) = 1,
    SetLimit(SetLimit) = 2,
    ChangeLimit(ChangeLimit) = 3,
    SetTransLimit(SetTransLimit) = 4,
    Deposit(Deposit) = 5,
    Withdraw(Withdraw) = 6,
    TransferOut(TransferOut) = 7,
    TransferIn(TransferIn) = 8,
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
