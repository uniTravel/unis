use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct OpenPeriod {
    account_code: String,
    period: String,
}

impl Command for OpenPeriod {
    type A = super::Transaction;
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
    type A = super::Transaction;

    fn apply(&self, agg: &mut Self::A) {
        agg.account_code = self.account_code.clone();
        agg.period = self.period.clone();
    }
}
