use super::*;
use crate::tests::*;
use proptest::prelude::*;
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest, prop_state_machine};
use unis::domain::{Aggregate, Command};

#[derive(Clone, Debug)]
pub struct RefTransaction {
    account_code: String,
    limit: i64,
    trans_limit: i64,
    balance: i64,
}

impl ReferenceStateMachine for RefTransaction {
    type State = RefTransaction;
    type Transition = TransactionCommand;

    fn init_state() -> BoxedStrategy<Self::State> {
        Just(RefTransaction {
            account_code: String::default(),
            limit: 0,
            trans_limit: 0,
            balance: 0,
        })
        .boxed()
    }

    fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
        match state {
            RefTransaction {
                account_code,
                limit: 0,
                trans_limit: 0,
                balance: 0,
            } if account_code.is_empty() => prop_oneof![
                init::tests::valid_com().prop_map(TransactionCommand::InitPeriod),
                open::tests::valid_com().prop_map(TransactionCommand::OpenPeriod)
            ]
            .boxed(),
            RefTransaction {
                account_code: _,
                limit: 0,
                trans_limit: 0,
                balance: 0,
            } => set_limit::tests::valid_com()
                .prop_map(TransactionCommand::SetLimit)
                .boxed(),
            RefTransaction {
                account_code: _,
                limit,
                trans_limit,
                balance: _,
            } if limit > &0 && trans_limit > &0 => prop_oneof![
                change_limit::tests::valid_com().prop_map(TransactionCommand::ChangeLimit),
                set_trans_limit::tests::valid_com().prop_map(TransactionCommand::SetTransLimit),
                deposit::tests::valid_com().prop_map(TransactionCommand::Deposit),
                withdraw::tests::valid_com().prop_map(TransactionCommand::Withdraw),
                transfer_in::tests::valid_com().prop_map(TransactionCommand::TransferIn),
                transfer_out::tests::valid_com().prop_map(TransactionCommand::TransferOut)
            ]
            .boxed(),
            _ => {
                println!("{:#?}", state);
                panic!("状态描述不完整");
            }
        }
    }

    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            TransactionCommand::InitPeriod(com) => {
                state.account_code = com.account_code.clone();
                state.limit = com.limit;
                state.trans_limit = com.limit;
                state
            }
            TransactionCommand::OpenPeriod(com) => {
                state.account_code = com.account_code.clone();
                state
            }
            TransactionCommand::SetLimit(com) => {
                state.limit = com.limit;
                state.trans_limit = com.trans_limit;
                state.balance = com.balance;
                state
            }
            TransactionCommand::ChangeLimit(com) => {
                state.limit = com.limit;
                if state.trans_limit > com.limit {
                    state.trans_limit = com.limit;
                }
                state
            }
            TransactionCommand::SetTransLimit(com) => {
                state.trans_limit = com.trans_limit;
                state
            }
            TransactionCommand::Deposit(com) => {
                state.balance += com.amount;
                state
            }
            TransactionCommand::Withdraw(com) => {
                state.balance -= com.amount;
                state
            }
            TransactionCommand::TransferOut(com) => {
                state.balance -= com.amount;
                state
            }
            TransactionCommand::TransferIn(com) => {
                state.balance += com.amount;
                state
            }
        }
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        match transition {
            TransactionCommand::ChangeLimit(com) => com.limit != state.limit,
            TransactionCommand::SetTransLimit(com) => {
                com.trans_limit <= state.limit && com.trans_limit != state.trans_limit
            }
            TransactionCommand::Deposit(com) => com.amount <= state.trans_limit,
            TransactionCommand::Withdraw(com) => {
                com.amount <= state.trans_limit && com.amount <= state.balance
            }
            TransactionCommand::TransferOut(com) => {
                com.amount <= state.trans_limit && com.amount <= state.balance
            }
            TransactionCommand::TransferIn(com) => com.amount <= state.trans_limit,
            _ => true,
        }
    }
}

impl StateMachineTest for Transaction {
    type SystemUnderTest = Transaction;
    type Reference = RefTransaction;

    fn init_test(
        _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) -> Self::SystemUnderTest {
        Transaction::new(uuid::Uuid::new_v4())
    }

    fn apply(
        mut state: Self::SystemUnderTest,
        _ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        transition: <Self::Reference as ReferenceStateMachine>::Transition,
    ) -> Self::SystemUnderTest {
        match transition {
            TransactionCommand::InitPeriod(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::OpenPeriod(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::SetLimit(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::ChangeLimit(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::SetTransLimit(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::Deposit(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::Withdraw(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::TransferOut(com) => {
                let _ = com.process(&mut state);
                state
            }
            TransactionCommand::TransferIn(com) => {
                let _ = com.process(&mut state);
                state
            }
        }
    }

    fn check_invariants(
        state: &Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) {
        assert_eq!(ref_state.account_code, state.account_code);
        assert_eq!(ref_state.limit, state.limit);
        assert_eq!(ref_state.trans_limit, state.trans_limit);
        assert_eq!(ref_state.balance, state.balance);
        assert!(state.balance >= 0)
    }
}

prop_state_machine! {
    #[test]
    fn transaction_state_machine(sequential 1..7 => Transaction);
}

proptest! {
    #![proptest_config(proptest_config())]

    #[test]
    fn state_transaction_init(
        set_limit in set_limit::tests::valid_com(),
        change_limit in change_limit::tests::valid_com(),
        set_trans_limit in set_trans_limit::tests::valid_com(),
        deposit in deposit::tests::valid_com(),
        withdraw in withdraw::tests::valid_com(),
        transfer_in in transfer_in::tests::valid_com(),
        transfer_out in transfer_out::tests::valid_com(),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());

        prop_assert!(set_limit.process(&mut agg).is_err());
        prop_assert!(change_limit.process(&mut agg).is_err());
        prop_assert!(set_trans_limit.process(&mut agg).is_err());
        prop_assert!(deposit.process(&mut agg).is_err());
        prop_assert!(withdraw.process(&mut agg).is_err());
        prop_assert!(transfer_in.process(&mut agg).is_err());
        prop_assert!(transfer_out.process(&mut agg).is_err());
    }

    #[test]
    fn state_transaction_opened(
        open_period in open::tests::valid_com(),
        change_limit in change_limit::tests::valid_com(),
        set_trans_limit in set_trans_limit::tests::valid_com(),
        deposit in deposit::tests::valid_com(),
        withdraw in withdraw::tests::valid_com(),
        transfer_in in transfer_in::tests::valid_com(),
        transfer_out in transfer_out::tests::valid_com(),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = open_period.process(&mut agg);

        prop_assert!(change_limit.process(&mut agg).is_err());
        prop_assert!(set_trans_limit.process(&mut agg).is_err());
        prop_assert!(deposit.process(&mut agg).is_err());
        prop_assert!(withdraw.process(&mut agg).is_err());
        prop_assert!(transfer_in.process(&mut agg).is_err());
        prop_assert!(transfer_out.process(&mut agg).is_err());
    }

    #[test]
    fn state_transaction_valid(
        init_period in init::tests::valid_com(),
        set_limit in set_limit::tests::valid_com(),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);

        prop_assert!(set_limit.process(&mut agg).is_err());
    }

}
