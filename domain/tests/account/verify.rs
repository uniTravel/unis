use crate::*;
use domain::account::VerifyAccount;
use proptest::prelude::*;

prop_compose! {
    fn valid_com() (
        verified_by in any::<String>(),
        conclusion in prop::bool::ANY
    ) -> VerifyAccount {
        VerifyAccount { verified_by, conclusion }
    }
}

proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn valid_command(com in valid_com()) {
        let result = unis::validate(&com, "zh");
        prop_assert!(result.is_ok());
    }
}
