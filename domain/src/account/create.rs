use crate::validate;
use unis::{
    domain::{Command, Event},
    errors::UniError,
    macros::{command, event},
};

#[command]
pub struct CreateAccount {
    #[validate(
        length(equal = 6, code = "exact_length"),
        custom(function = "validate::code")
    )]
    #[schema(pattern = r"^\d{6}$")]
    /// 账号
    pub code: String,
    #[validate(length(min = 1))]
    #[schema(min_length = 1)]
    /// 申请人
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

#[cfg(feature = "test-utils")]
proptest::prop_compose! {
    pub fn create() (
        code in crate::tests::digit_string(6),
        owner in crate::tests::long_string(1)
    ) -> CreateAccount {
        CreateAccount { code, owner }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::tests::*;
    use proptest::prelude::*;

    prop_compose! {
        fn invalid_code_length() (
            code in prop_oneof![short_string(5), long_string(7)],
            owner in long_string(1)
        ) -> CreateAccount {
            CreateAccount { code, owner }
        }
    }

    prop_compose! {
        fn invalid_code_type() (
            code in prop::string::string_regex("[^0-9]{6}").unwrap(),
            owner in long_string(1)
        ) -> CreateAccount {
            CreateAccount { code, owner }
        }
    }

    proptest! {
        #[test]
        fn valid_command(com in create()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_ok());
        }

        #[test]
        fn code_length(com in invalid_code_length()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("长度应为 6"));
        }

        #[test]
        fn code_type(com in invalid_code_type()) {
            let result = unis::validate(&com, "zh");
            prop_assert!(result.is_err());
            prop_assert!(result.unwrap_err().contains("应为 ASCII 数字"));
        }
    }
}
