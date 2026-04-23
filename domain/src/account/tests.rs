use super::*;
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

impl RefAccount {
    fn can_create(&self) -> bool {
        self.code.is_empty() && self.verified_by.is_empty() && self.approved_by.is_empty()
    }

    fn can_verify(&self) -> bool {
        !self.code.is_empty() && self.verified_by.is_empty() && self.approved_by.is_empty()
    }

    fn can_approve(&self) -> bool {
        !self.code.is_empty() && !self.verified_by.is_empty() && self.approved_by.is_empty()
    }

    fn can_limit(&self) -> bool {
        !self.code.is_empty() && !self.verified_by.is_empty() && !self.approved_by.is_empty()
    }

    fn can_transition(&self, transition: &AccountCommand) -> bool {
        match transition {
            AccountCommand::Create(_) => self.can_create(),
            AccountCommand::Verify(_) => self.can_verify(),
            AccountCommand::Approve(_) => self.can_approve(),
            AccountCommand::Limit(_) => self.can_limit(),
        }
    }
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
            } if state.can_create() => create().prop_map(AccountCommand::Create).boxed(),
            RefAccount {
                code: _,
                limit: 0,
                verified_by,
                verified: false,
                approved_by,
                approved: false,
            } if state.can_verify() => verify().prop_map(AccountCommand::Verify).boxed(),
            RefAccount {
                code: _,
                limit: 0,
                verified_by: _,
                verified: _,
                approved_by,
                approved: false,
            } if state.can_approve() => approve().prop_map(AccountCommand::Approve).boxed(),
            RefAccount {
                code: _,
                limit: _,
                verified_by: _,
                verified: true,
                approved_by,
                approved: _,
            } if state.can_limit() => limit().prop_map(AccountCommand::Limit).boxed(),
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
            }
            AccountCommand::Verify(com) => {
                state.verified_by = com.verified_by.clone();
                state.verified = com.verified;
            }
            AccountCommand::Approve(com) => {
                if state.verified {
                    state.approved_by = com.approved_by.clone();
                    state.approved = com.approved;
                    state.limit = com.limit;
                }
            }
            AccountCommand::Limit(com) => {
                if state.approved {
                    state.limit = com.limit;
                }
            }
        }
        state
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        (match transition {
            AccountCommand::Limit(com) => com.limit != state.limit,
            _ => true,
        }) && state.can_transition(transition)
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
            }
            AccountCommand::Verify(com) => {
                let _ = com.process(&mut state);
            }
            AccountCommand::Approve(com) => {
                if ref_state.verified {
                    let _ = com.process(&mut state);
                }
            }
            AccountCommand::Limit(com) => {
                if ref_state.approved {
                    let _ = com.process(&mut state);
                }
            }
        }
        state
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
    #[test]
    fn start_account(
        verify_account in verify(),
        approve_account in approve(),
        limit_account in limit(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(approve_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn stop_account_verified_false(
        create_account in create(),
        verify_account in verify(),
        approve_account in approve(),
        limit_account in limit(),
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
    fn stop_account_approved_false(
        create_account in create(),
        verify_account in verify(),
        approve_account in approve(),
        limit_account in limit(),
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

    #[test]
    fn state_account_created(
        create_account in create(),
        approve_account in approve(),
        limit_account in limit(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());

        prop_assert!(approve_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn state_account_verified_true(
        create_account in create(),
        verify_account in verify(),
        limit_account in limit(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = true;

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(limit_account.process(&mut agg).is_err());
    }

    #[test]
    fn state_account_approved_true(
        create_account in create(),
        verify_account in verify(),
        approve_account in approve(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = true;
        prop_assert!(approve_account.clone().process(&mut agg).is_ok());
        agg.approved = true;

        prop_assert!(verify_account.process(&mut agg).is_err());
        prop_assert!(approve_account.process(&mut agg).is_err());
    }

    #[test]
    fn limit_account_restrict(
        create_account in create(),
        verify_account in verify(),
        approve_account in approve(),
        mut limit_account in limit(),
    ) {
        let mut agg = Account::new(uuid::Uuid::new_v4());
        prop_assert!(create_account.process(&mut agg).is_ok());
        prop_assert!(verify_account.clone().process(&mut agg).is_ok());
        agg.verified = true;
        prop_assert!(approve_account.clone().process(&mut agg).is_ok());
        agg.approved = true;

        limit_account.limit = agg.limit;
        prop_assert!(limit_account.process(&mut agg).is_err());
    }
}
