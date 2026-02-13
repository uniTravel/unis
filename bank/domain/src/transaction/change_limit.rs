use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct ChangeLimit {
    pub limit: i64,
}

impl Command for ChangeLimit {
    type A = super::Transaction;
    type E = LimitChanged;

    fn check(&self, agg: &Self::A) -> Result<(), UniError> {
        if agg.limit == 0 {
            return Err(UniError::CheckError("待修改限额须大于零".to_string()));
        }

        Ok(())
    }

    fn apply(self, agg: &Self::A) -> Self::E {
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
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.limit = self.limit;
        agg.trans_limit = self.trans_limit;
    }
}
