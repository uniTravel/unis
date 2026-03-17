use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct SetLimit {
    pub limit: i64,
    pub trans_limit: i64,
    pub balance: i64,
}

impl Command for SetLimit {
    type A = super::Transaction;
    type E = LimitSetted;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit != 0 {
            return Err(UniError::CheckError("不得重复设置限额".to_string()));
        }

        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
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
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
        agg.trans_limit = self.trans_limit;
        agg.balance = self.balance;
    }
}
