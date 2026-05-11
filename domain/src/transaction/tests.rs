use crate::{
    tests::{less_great, less_great_equal},
    transaction::{
        Transaction, TransactionCommand, change_limit, deposit, init, open,
        set_limit, set_trans_limit, transfer_in, transfer_out,
        withdraw,
    },
};
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

impl RefTransaction {
    fn can_create(&self) -> bool {
        self.account_code.is_empty()
    }

    fn can_set_limit(&self) -> bool {
        !self.account_code.is_empty() && self.limit == 0
    }

    fn can_transaction(&self) -> bool {
        self.limit > 0 && self.trans_limit > 0
    }

    fn can_transition(&self, transition: &TransactionCommand) -> bool {
        match transition {
            TransactionCommand::InitPeriod(_) => self.can_create(),
            TransactionCommand::OpenPeriod(_) => self.can_create(),
            TransactionCommand::SetLimit(_) => self.can_set_limit(),
            TransactionCommand::ChangeLimit(_) => self.can_transaction(),
            TransactionCommand::SetTransLimit(_) => self.can_transaction(),
            TransactionCommand::Deposit(_) => self.can_transaction(),
            TransactionCommand::Withdraw(_) => self.can_transaction(),
            TransactionCommand::TransferOut(_) => self.can_transaction(),
            TransactionCommand::TransferIn(_) => self.can_transaction(),
        }
    }
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
            } if state.can_create() => prop_oneof![
                init().prop_map(TransactionCommand::InitPeriod),
                open().prop_map(TransactionCommand::OpenPeriod)
            ]
            .boxed(),
            RefTransaction {
                account_code: _,
                limit: 0,
                trans_limit: 0,
                balance: 0,
            } if state.can_set_limit() => set_limit()
                .prop_map(TransactionCommand::SetLimit)
                .boxed(),
            RefTransaction {
                account_code: _,
                limit,
                trans_limit,
                balance: _,
            } if state.can_transaction() => prop_oneof![
                change_limit().prop_map(TransactionCommand::ChangeLimit),
                set_trans_limit().prop_map(TransactionCommand::SetTransLimit),
                deposit().prop_map(TransactionCommand::Deposit),
                withdraw().prop_map(TransactionCommand::Withdraw),
                transfer_in().prop_map(TransactionCommand::TransferIn),
                transfer_out().prop_map(TransactionCommand::TransferOut)
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
            }
            TransactionCommand::OpenPeriod(com) => {
                state.account_code = com.account_code.clone();
            }
            TransactionCommand::SetLimit(com) => {
                state.limit = com.limit;
                state.trans_limit = com.trans_limit;
                state.balance = com.balance;
            }
            TransactionCommand::ChangeLimit(com) => {
                state.limit = com.limit;
                if state.trans_limit > com.limit {
                    state.trans_limit = com.limit;
                }
            }
            TransactionCommand::SetTransLimit(com) => {
                state.trans_limit = com.trans_limit;
            }
            TransactionCommand::Deposit(com) => {
                state.balance += com.amount;
            }
            TransactionCommand::Withdraw(com) => {
                state.balance -= com.amount;
            }
            TransactionCommand::TransferOut(com) => {
                state.balance -= com.amount;
            }
            TransactionCommand::TransferIn(com) => {
                state.balance += com.amount;
            }
        }
        state
    }

    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        (match transition {
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
        }) && state.can_transition(transition)
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
            }
            TransactionCommand::OpenPeriod(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::SetLimit(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::ChangeLimit(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::SetTransLimit(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::Deposit(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::Withdraw(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::TransferOut(com) => {
                let _ = com.process(&mut state);
            }
            TransactionCommand::TransferIn(com) => {
                let _ = com.process(&mut state);
            }
        }
        state
    }

    fn check_invariants(
        state: &Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) {
        assert_eq!(ref_state.account_code, state.account_code);
        assert_eq!(ref_state.limit, state.limit);
        assert_eq!(ref_state.trans_limit, state.trans_limit);
        assert_eq!(ref_state.balance, state.balance);
        assert!(state.trans_limit <= state.limit);
        assert!(state.balance >= 0)
    }
}

prop_state_machine! {
    #[test]
    fn transaction_state_machine(sequential 1..7 => Transaction);
}

proptest! {
    #[test]
    fn start_transaction(
        set_limit in set_limit(),
        change_limit in change_limit(),
        set_trans_limit in set_trans_limit(),
        deposit in deposit(),
        withdraw in withdraw(),
        transfer_in in transfer_in(),
        transfer_out in transfer_out(),
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
        open_period in open(),
        change_limit in change_limit(),
        set_trans_limit in set_trans_limit(),
        deposit in deposit(),
        withdraw in withdraw(),
        transfer_in in transfer_in(),
        transfer_out in transfer_out(),
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
        init_period in init(),
        set_limit in set_limit(),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);

        prop_assert!(set_limit.process(&mut agg).is_err());
    }

    #[test]
    fn change_limit_restrict(
        init_period in init(),
        mut change_limit in change_limit(),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);
        change_limit.limit = agg.limit;

        prop_assert!(change_limit.process(&mut agg).is_err());
    }

    #[test]
    fn set_trans_limit_restrict(
        init_period in init(),
        mut set_trans_limit in set_trans_limit(),
        (l, t) in less_great_equal(10_000..10_000_000i64, 10_000..10_000_000i64),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);
        let mut set_trans_limit_c = set_trans_limit.clone();
        set_trans_limit_c.trans_limit = agg.trans_limit;

        prop_assert!(set_trans_limit_c.process(&mut agg).is_err());

        agg.limit = l;
        set_trans_limit.trans_limit = t;

        prop_assert!(set_trans_limit.process(&mut agg).is_err());
    }

    #[test]
    fn deposit_restrict(
        init_period in init(),
        mut deposit in deposit(),
        (t, a) in less_great(10_000..10_000_000i64, 1..i64::MAX),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);
        agg.trans_limit = t;
        deposit.amount = a;

        prop_assert!(deposit.process(&mut agg).is_err());
    }

    #[test]
    fn withdraw_restrict(
        init_period in init(),
        mut withdraw in withdraw(),
        (t, a1) in less_great(10_000..10_000_000i64, 1..i64::MAX),
        (b, a2) in less_great(0..i64::MAX, 1..i64::MAX),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);
        let mut agg_c = agg.clone();
        let mut withdraw_c = withdraw.clone();
        agg_c.trans_limit = t;
        withdraw_c.amount = a1;

        prop_assert!(withdraw_c.process(&mut agg_c).is_err());

        agg.balance = b;
        withdraw.amount = a2;

        prop_assert!(withdraw.process(&mut agg).is_err());
    }

    #[test]
    fn transfer_in_restrict(
        init_period in init(),
        mut transfer_in in transfer_in(),
        (t, a) in less_great(10_000..10_000_000i64, 1..i64::MAX),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);
        agg.trans_limit = t;
        transfer_in.amount = a;

        prop_assert!(transfer_in.process(&mut agg).is_err());
    }

    #[test]
    fn transfer_out_restrict(
        init_period in init(),
        mut transfer_out in transfer_out(),
        (t, a1) in less_great(10_000..10_000_000i64, 1..i64::MAX),
        (b, a2) in less_great(0..i64::MAX, 1..i64::MAX),
    ) {
        let mut agg = Transaction::new(uuid::Uuid::new_v4());
        let _ = init_period.process(&mut agg);
        let mut agg_c = agg.clone();
        let mut transfer_out_c = transfer_out.clone();
        agg_c.trans_limit = t;
        transfer_out_c.amount = a1;
        prop_assert!(transfer_out_c.process(&mut agg_c).is_err());

        agg.balance = b;
        transfer_out.amount = a2;

        prop_assert!(transfer_out.process(&mut agg).is_err());
    }
}
