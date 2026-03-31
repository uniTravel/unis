use super::*;
use crate::tests::*;
use proptest::prelude::*;
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};
use unis::domain::{Aggregate, Command};

#[derive(Clone, Debug)]
pub struct RefAccount {
    code: String,
    limit: i64,
    verified_by: String,
    verified: bool,
    approved_by: String,
    approved: bool,
}

impl ReferenceStateMachine for RefAccount {
    type State = RefAccount;
    type Transition = AccountCommand;

    fn init_state() -> BoxedStrategy<Self::State> {
        Just(RefAccount {
            code: String::default(),
            limit: 0,
            verified_by: String::default(),
            verified: false,
            approved_by: String::default(),
            approved: false,
        })
        .boxed()
    }

    fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
        match state {
            RefAccount {
                code,
                limit: 0,
                verified_by,
                verified: false,
                approved_by,
                approved: false,
            } if code.is_empty() && verified_by.is_empty() && approved_by.is_empty() => {
                create::tests::valid_com()
                    .prop_map(AccountCommand::Create)
                    .boxed()
            }
            RefAccount {
                code: _,
                limit: 0,
                verified_by,
                verified: false,
                approved_by,
                approved: false,
            } if verified_by.is_empty() && approved_by.is_empty() => verify::tests::valid_com()
                .prop_map(AccountCommand::Verify)
                .boxed(),
            RefAccount {
                code: _,
                limit: 0,
                verified_by: _,
                verified: _,
                approved_by,
                approved: false,
            } if approved_by.is_empty() => approve::tests::valid_com()
                .prop_map(AccountCommand::Approve)
                .boxed(),
            RefAccount {
                code: _,
                limit: _,
                verified_by: _,
                verified: true,
                approved_by,
                approved: _,
            } if !approved_by.is_empty() => limit::tests::valid_com()
                .prop_map(AccountCommand::Limit)
                .boxed(),
            _ => {
                println!("{:#?}", state);
                panic!("状态描述不完整");
            }
        }
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            AccountCommand::Create(com) => {
                state.code = com.code.clone();
                state
            }
            AccountCommand::Verify(com) => {
                state.verified_by = com.verified_by.clone();
                state.verified = com.verified;
                state
            }
            AccountCommand::Approve(com) => {
                if state.verified {
                    state.approved_by = com.approved_by.clone();
                    state.approved = com.approved;
                    state.limit = com.limit;
                }
                state
            }
            AccountCommand::Limit(com) => {
                if state.approved {
                    state.limit = com.limit;
                }
                state
            }
        }
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        match transition {
            AccountCommand::Limit(com) => com.limit != state.limit,
            _ => true,
        }
    }
}

impl StateMachineTest for Account {
    type SystemUnderTest = Account;
    type Reference = RefAccount;

    fn init_test(
        _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) -> Self::SystemUnderTest {
        Account::new(uuid::Uuid::new_v4())
    }

    fn apply(
        mut state: Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        transition: <Self::Reference as ReferenceStateMachine>::Transition,
    ) -> Self::SystemUnderTest {
        match transition {
            AccountCommand::Create(com) => {
                let _ = com.process(&mut state);
                state
            }
            AccountCommand::Verify(com) => {
                let _ = com.process(&mut state);
                state
            }
            AccountCommand::Approve(com) => {
                if ref_state.verified {
                    let _ = com.process(&mut state);
                }
                state
            }
            AccountCommand::Limit(com) => {
                if ref_state.approved {
                    let _ = com.process(&mut state);
                }
                state
            }
        }
    }

    fn check_invariants(
        state: &Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) {
        assert_eq!(ref_state.code, state.code);
        assert_eq!(ref_state.verified_by, state.verified_by);
        assert_eq!(ref_state.verified, state.verified);
        assert_eq!(ref_state.approved_by, state.approved_by);
        assert_eq!(ref_state.approved, state.approved);
        assert_eq!(ref_state.limit, state.limit);
    }
}

prop_state_machine! {
    #[test]
    fn account_state_machine(sequential 1..7 => Account);
}

proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn state_account_init(
        verify_account in verify::tests::valid_com(),
        approve_account in approve::tests::valid_com(),
        limit_account in limit::tests::valid_com(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(approve_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn state_account_created(
        create_account in create::tests::valid_com(),
        approve_account in approve::tests::valid_com(),
        limit_account in limit::tests::valid_com(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());

        prop_assert!(approve_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn state_account_verified_true(
        create_account in create::tests::valid_com(),
        verify_account in verify::tests::valid_com(),
        limit_account in limit::tests::valid_com(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = true;

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn state_account_verified_false(
        create_account in create::tests::valid_com(),
        verify_account in verify::tests::valid_com(),
        approve_account in approve::tests::valid_com(),
        limit_account in limit::tests::valid_com(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = false;

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(approve_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn state_account_approved_true(
        create_account in create::tests::valid_com(),
        verify_account in verify::tests::valid_com(),
        approve_account in approve::tests::valid_com(),
        limit_account in limit::tests::valid_com(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = true;
        prop_assert!(approve_account.clone().process(&mut agg).is_ok());
        agg.approved = true;

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(approve_account.process(&mut agg).is_err());

        if limit_account.limit == agg.limit {
            prop_assert!(limit_account.process(&mut agg).is_err());
        } else {
            prop_assert!(limit_account.process(&mut agg).is_ok());
        }
    }

    #[test]
    fn state_account_approved_false(
        create_account in create::tests::valid_com(),
        verify_account in verify::tests::valid_com(),
        approve_account in approve::tests::valid_com(),
        limit_account in limit::tests::valid_com(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = true;
        prop_assert!(approve_account.clone().process(&mut agg).is_ok());
        agg.approved = false;

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(approve_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }
}
