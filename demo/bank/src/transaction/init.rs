use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct InitPeriod {
    account_code: String,
    limit: i64,
}

impl Command for InitPeriod {
    type A = super::Transaction;
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
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.account_code = self.account_code.clone();
        agg.period = self.period.clone();
        agg.limit = self.limit;
    }
}
