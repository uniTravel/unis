use super::*;
use chrono::Local;
use domain::transaction::*;

#[fixture]
async fn app() -> &'static Router {
    static CELL: OnceCell<Router> = OnceCell::const_new();
    CELL.get_or_init(|| async {
        LazyLock::force(&SETUP);
        let svc = Arc::new(
            super::ctx()
                .setup::<_, KafkaSender<TransactionCommand>>()
                .await,
        );
        Router::new().merge(routes::transaction_routes().with_state(svc))
    })
    .await
}

const PATH: &str = "/api/v1/rkyv/transaction";

#[derive(Clone, Debug)]
struct RefTransaction {
    account_code: String,
    period: String,
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
            period: String::default(),
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
                period: _,
                limit: 0,
                trans_limit: 0,
                balance: 0,
            } if state.account_code.is_empty() => prop_oneof![
                init().prop_map(TransactionCommand::InitPeriod),
                open().prop_map(TransactionCommand::OpenPeriod)
            ]
            .boxed(),
            RefTransaction {
                account_code: _,
                period: _,
                limit: 0,
                trans_limit: 0,
                balance: 0,
            } => set_limit().prop_map(TransactionCommand::SetLimit).boxed(),
            RefTransaction {
                account_code: _,
                period: _,
                limit,
                trans_limit,
                balance: _,
            } if state.limit > 0 && state.trans_limit > 0 => prop_oneof![
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
                state.period = Local::now().format("%Y-%m").to_string();
                state.limit = com.limit;
                state.trans_limit = com.limit;
            }
            TransactionCommand::OpenPeriod(com) => {
                state.account_code = com.account_code.clone();
                state.period = Local::now().format("%Y-%m").to_string();
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

fn process(body: Bytes, mut state: Transaction) -> Transaction {
    match TransactionEvent::from_bytes(&body).unwrap() {
        TransactionEvent::PeriodInited(evt) => evt.process(&mut state),
        TransactionEvent::PeriodOpened(evt) => evt.process(&mut state),
        TransactionEvent::LimitSetted(evt) => evt.process(&mut state),
        TransactionEvent::LimitChanged(evt) => evt.process(&mut state),
        TransactionEvent::TransLimitSetted(evt) => evt.process(&mut state),
        TransactionEvent::DepositFinished(evt) => evt.process(&mut state),
        TransactionEvent::WithdrawFinished(evt) => evt.process(&mut state),
        TransactionEvent::TransferOutFinished(evt) => evt.process(&mut state),
        TransactionEvent::TransferInFinished(evt) => evt.process(&mut state),
    }
    state
}

#[rstest]
#[tokio::test]
async fn transaction_state_machine(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let (mut ref_state, transactions, _) = RefTransaction::sequential_strategy(1..7)
        .new_tree(&mut TestRunner::default())
        .unwrap()
        .current();
    let mut state = Transaction::new(Uuid::new_v4());

    for transition in transactions {
        ref_state = RefTransaction::apply(ref_state, &transition);
        match transition {
            TransactionCommand::InitPeriod(com) => {
                let (_, agg_id, body) = sender::create(app, PATH, "init", com).await;
                state = process(body, Transaction::new(agg_id));
            }
            TransactionCommand::OpenPeriod(com) => {
                let (_, agg_id, body) = sender::create(app, PATH, "open", com).await;
                state = process(body, Transaction::new(agg_id));
            }
            TransactionCommand::SetLimit(com) => {
                let (_, body) = sender::change(app, PATH, "set_limit", state.id(), com).await;
                state = process(body, state);
            }
            TransactionCommand::ChangeLimit(com) => {
                let (_, body) = sender::change(app, PATH, "change_limit", state.id(), com).await;
                state = process(body, state);
            }
            TransactionCommand::SetTransLimit(com) => {
                let (_, body) = sender::change(app, PATH, "set_trans_limit", state.id(), com).await;
                state = process(body, state);
            }
            TransactionCommand::Deposit(com) => {
                let (_, body) = sender::change(app, PATH, "deposit", state.id(), com).await;
                state = process(body, state);
            }
            TransactionCommand::Withdraw(com) => {
                let (_, body) = sender::change(app, PATH, "withdraw", state.id(), com).await;
                state = process(body, state);
            }
            TransactionCommand::TransferOut(com) => {
                let (_, body) = sender::change(app, PATH, "transfer_out", state.id(), com).await;
                state = process(body, state);
            }
            TransactionCommand::TransferIn(com) => {
                let (_, body) = sender::change(app, PATH, "transfer_in", state.id(), com).await;
                state = process(body, state);
            }
        }
    }

    assert_eq!(ref_state.account_code, state.account_code);
    assert_eq!(ref_state.period, state.period);
    assert_eq!(ref_state.limit, state.limit);
    assert_eq!(ref_state.trans_limit, state.trans_limit);
    assert_eq!(ref_state.balance, state.balance);
    assert!(state.trans_limit <= state.limit);
    assert!(state.balance >= 0);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_transaction_opened(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let open = open().new_tree(&mut runner).unwrap().current();
    let change_limit = change_limit().new_tree(&mut runner).unwrap().current();
    let set_trans_limit = set_trans_limit().new_tree(&mut runner).unwrap().current();
    let deposit = deposit().new_tree(&mut runner).unwrap().current();
    let withdraw = withdraw().new_tree(&mut runner).unwrap().current();
    let transfer_in = transfer_in().new_tree(&mut runner).unwrap().current();
    let transfer_out = transfer_out().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, _) = sender::create(app, PATH, "open", open).await;

    let (s, _) = sender::change(app, PATH, "change_limit", agg_id, change_limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = sender::change(app, PATH, "set_trans_limit", agg_id, set_trans_limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = sender::change(app, PATH, "deposit", agg_id, deposit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = sender::change(app, PATH, "withdraw", agg_id, withdraw).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = sender::change(app, PATH, "transfer_in", agg_id, transfer_in).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = sender::change(app, PATH, "transfer_out", agg_id, transfer_out).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_transaction_valid(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let init = init().new_tree(&mut runner).unwrap().current();
    let set_limit = set_limit().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, _) = sender::create(app, PATH, "init", init).await;

    let (s, _) = sender::change(app, PATH, "set_limit", agg_id, set_limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn change_limit_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let init = init().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, body) = sender::create(app, PATH, "init", init).await;
    let mut state = Transaction::new(agg_id);
    state = process(body, state);
    let mut com = change_limit().new_tree(&mut runner).unwrap().current();
    com.limit = state.limit;

    let (s, _) = sender::change(app, PATH, "change_limit", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn set_trans_limit_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let init = init().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, body) = sender::create(app, PATH, "init", init).await;
    let mut state = Transaction::new(agg_id);
    state = process(body, state);
    let mut com = set_trans_limit().new_tree(&mut runner).unwrap().current();
    com.trans_limit = state.trans_limit;

    let (s, _) = sender::change(app, PATH, "set_trans_limit", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    if state.limit < 10_000_000 {
        let mut com = set_trans_limit().new_tree(&mut runner).unwrap().current();
        com.trans_limit = (state.limit..10_000_000)
            .new_tree(&mut runner)
            .unwrap()
            .current();

        let (s, _) = sender::change(app, PATH, "set_trans_limit", agg_id, com).await;
        assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn deposit_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let open = open().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, body) = sender::create(app, PATH, "open", open).await;
    let mut state = Transaction::new(agg_id);
    state = process(body, state);
    let set_limit = set_limit().new_tree(&mut runner).unwrap().current();
    let (_, body) = sender::change(app, PATH, "set_limit", agg_id, set_limit).await;
    state = process(body, state);
    let mut com = deposit().new_tree(&mut runner).unwrap().current();
    com.amount = (state.trans_limit + 1..)
        .new_tree(&mut runner)
        .unwrap()
        .current();

    let (s, _) = sender::change(app, PATH, "deposit", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn withdraw_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let open = open().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, body) = sender::create(app, PATH, "open", open).await;
    let mut state = Transaction::new(agg_id);
    state = process(body, state);
    let set_limit = set_limit().new_tree(&mut runner).unwrap().current();
    let (_, body) = sender::change(app, PATH, "set_limit", agg_id, set_limit).await;
    state = process(body, state);
    let mut com = withdraw().new_tree(&mut runner).unwrap().current();
    com.amount = (state.trans_limit + 1..)
        .new_tree(&mut runner)
        .unwrap()
        .current();

    let (s, _) = sender::change(app, PATH, "withdraw", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    let mut com = withdraw().new_tree(&mut runner).unwrap().current();
    com.amount = (state.balance + 1..)
        .new_tree(&mut runner)
        .unwrap()
        .current();

    let (s, _) = sender::change(app, PATH, "withdraw", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn transfer_in_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let open = open().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, body) = sender::create(app, PATH, "open", open).await;
    let mut state = Transaction::new(agg_id);
    state = process(body, state);
    let set_limit = set_limit().new_tree(&mut runner).unwrap().current();
    let (_, body) = sender::change(app, PATH, "set_limit", agg_id, set_limit).await;
    state = process(body, state);
    let mut com = transfer_in().new_tree(&mut runner).unwrap().current();
    com.amount = (state.trans_limit + 1..)
        .new_tree(&mut runner)
        .unwrap()
        .current();

    let (s, _) = sender::change(app, PATH, "transfer_in", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn transfer_out_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let open = open().new_tree(&mut runner).unwrap().current();
    let (_, agg_id, body) = sender::create(app, PATH, "open", open).await;
    let mut state = Transaction::new(agg_id);
    state = process(body, state);
    let set_limit = set_limit().new_tree(&mut runner).unwrap().current();
    let (_, body) = sender::change(app, PATH, "set_limit", agg_id, set_limit).await;
    state = process(body, state);
    let mut com = transfer_out().new_tree(&mut runner).unwrap().current();
    com.amount = (state.trans_limit + 1..)
        .new_tree(&mut runner)
        .unwrap()
        .current();

    let (s, _) = sender::change(app, PATH, "transfer_out", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    let mut com = transfer_out().new_tree(&mut runner).unwrap().current();
    com.amount = (state.balance + 1..)
        .new_tree(&mut runner)
        .unwrap()
        .current();

    let (s, _) = sender::change(app, PATH, "transfer_out", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}
