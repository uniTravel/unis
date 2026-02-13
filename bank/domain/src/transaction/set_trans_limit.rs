use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct SetTransLimit {
    pub trans_limit: i64,
}

impl Command for SetTransLimit {
    type A = super::Transaction;
    type E = TransLimitSetted;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if self.trans_limit > agg.limit {
            return Err(UniError::CheckError("交易限额不得超过控制限额".to_string()));
        }
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
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
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.trans_limit = self.trans_limit;
    }
}
