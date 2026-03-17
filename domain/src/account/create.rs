use regex::Regex;
use std::sync::LazyLock;
use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

static CODE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9]{6}$").unwrap());

#[command]
pub struct CreateAccount {
    #[validate(regex(path = CODE_RE))]
    pub code: String,
    pub owner: String,
}

impl Command for CreateAccount {
    type A = super::Account;
    type E = AccountCreated;

    fn check(&self, _agg: &Self::A) -> Result<(), UniError> {
        Ok(())
    }

    fn apply(self, _agg: &Self::A) -> Self::E {
        Self::E {
            code: self.code,
            owner: self.owner,
        }
    }
}

#[event]
pub struct AccountCreated {
    code: String,
    owner: String,
}

impl Event for AccountCreated {
    type A = super::Account;

    fn apply(&self, agg: &mut Self::A) {
        agg.code = self.code.clone();
        agg.owner = self.owner.clone();
    }
}
