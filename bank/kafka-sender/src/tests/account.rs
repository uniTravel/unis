use super::*;
use domain::account::*;

#[fixture]
async fn app() -> &'static Router {
    static CELL: OnceCell<Router> = OnceCell::const_new();
    CELL.get_or_init(|| async {
        LazyLock::force(&SETUP);
        let svc = Arc::new(super::ctx().setup::<_, KafkaSender<AccountCommand>>().await);
        Router::new().merge(routes::account_routes().with_state(svc))
    })
    .await
}

const COUNT: usize = 256;
const PATH: &str = "/api/v1/rkyv/account";

prop_compose! {
    fn valid_create() (
        code in digit_string(6),
        owner in long_string(1)
    ) -> CreateAccount {
        CreateAccount { code, owner }
    }
}

prop_compose! {
    fn valid_verify() (
        verified_by in long_string(1),
        verified in prop::bool::ANY
    ) -> VerifyAccount {
        VerifyAccount { verified_by, verified }
    }
}

prop_compose! {
    fn true_verify() (
        verified_by in long_string(1)
    ) -> VerifyAccount {
        VerifyAccount { verified_by, verified: true }
    }
}

prop_compose! {
    fn false_verify() (
        verified_by in long_string(1)
    ) -> VerifyAccount {
        VerifyAccount { verified_by, verified: false }
    }
}

prop_compose! {
    fn valid_approve() (
        approved_by in long_string(1),
        approved in prop::bool::ANY,
        limit in 10_000..10_000_000i64
    ) -> ApproveAccount {
        ApproveAccount { approved_by, approved, limit }
    }
}

prop_compose! {
    fn true_approve() (
        approved_by in long_string(1),
        limit in 10_000..10_000_000i64
    ) -> ApproveAccount {
        ApproveAccount { approved_by, approved: true, limit }
    }
}

prop_compose! {
    fn false_approve() (
        approved_by in long_string(1),
        limit in 10_000..10_000_000i64
    ) -> ApproveAccount {
        ApproveAccount { approved_by, approved: false, limit }
    }
}

prop_compose! {
    fn valid_limit() (
        limit in 10_000..10_000_000i64
    ) -> LimitAccount {
        LimitAccount { limit }
    }
}

#[derive(Clone, Debug)]
pub struct RefAccount {
    code: String,
    owner: String,
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
            owner: String::default(),
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
                owner: _,
                limit: 0,
                verified_by,
                verified: false,
                approved_by,
                approved: false,
            } if code.is_empty() && verified_by.is_empty() && approved_by.is_empty() => {
                valid_create().prop_map(AccountCommand::Create).boxed()
            }
            RefAccount {
                code: _,
                owner: _,
                limit: 0,
                verified_by,
                verified: false,
                approved_by,
                approved: false,
            } if verified_by.is_empty() && approved_by.is_empty() => {
                valid_verify().prop_map(AccountCommand::Verify).boxed()
            }
            RefAccount {
                code: _,
                owner: _,
                limit: 0,
                verified_by: _,
                verified: _,
                approved_by,
                approved: false,
            } if approved_by.is_empty() => {
                valid_approve().prop_map(AccountCommand::Approve).boxed()
            }
            RefAccount {
                code: _,
                owner: _,
                limit: _,
                verified_by: _,
                verified: true,
                approved_by,
                approved: _,
            } if !approved_by.is_empty() => valid_limit().prop_map(AccountCommand::Limit).boxed(),
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
                state.owner = com.owner.clone();
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

fn process(evt: AccountEvent, state: &mut Account) {
    match evt {
        AccountEvent::Created(evt) => evt.process(state),
        AccountEvent::Verified(evt) => evt.process(state),
        AccountEvent::Approved(evt) => evt.process(state),
        AccountEvent::Limited(evt) => evt.process(state),
    }
}

#[rstest]
#[tokio::test]
async fn account_state_machine(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut tasks = JoinSet::new();
    let strategy = Arc::new(RefAccount::sequential_strategy(2..6));

    for _ in 0..COUNT {
        let strategy = strategy.clone();
        tasks.spawn(async move {
            let (mut ref_state, transitions, _) = strategy
                .new_tree(&mut TestRunner::default())
                .unwrap()
                .current();
            let mut state = Account::new(Uuid::new_v4());

            for transition in transitions {
                ref_state = RefAccount::apply(ref_state, &transition);
                match transition {
                    AccountCommand::Create(com) => {
                        let (_, agg_id, body) = create(app, PATH, "create", com).await;
                        let evt = AccountEvent::from_bytes(&body).unwrap();
                        let mut agg = Account::new(agg_id);
                        process(evt, &mut agg);
                        state = agg;
                    }
                    AccountCommand::Verify(com) => {
                        let (_, body) = change(app, PATH, "verify", state.id(), com).await;
                        process(AccountEvent::from_bytes(&body).unwrap(), &mut state);
                    }
                    AccountCommand::Approve(com) => {
                        if ref_state.verified {
                            let (_, body) = change(app, PATH, "approve", state.id(), com).await;
                            process(AccountEvent::from_bytes(&body).unwrap(), &mut state);
                        }
                    }
                    AccountCommand::Limit(com) => {
                        if ref_state.approved {
                            let (_, body) = change(app, PATH, "limit", state.id(), com).await;
                            process(AccountEvent::from_bytes(&body).unwrap(), &mut state);
                        }
                    }
                }
            }

            prop_assert_eq!(ref_state.code, state.code);
            prop_assert_eq!(ref_state.owner, state.owner);
            prop_assert_eq!(ref_state.verified_by, state.verified_by);
            prop_assert_eq!(ref_state.verified, state.verified);
            prop_assert_eq!(ref_state.approved_by, state.approved_by);
            prop_assert_eq!(ref_state.approved, state.approved);
            prop_assert_eq!(ref_state.limit, state.limit);
            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn stop_account_verified_false(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let false_verify = false_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let (_, agg_id, _) = create(app, PATH, "create", com_create).await;
            change(app, PATH, "verify", agg_id, false_verify).await;

            let (s, _) = change(app, PATH, "verify", agg_id, com_verify).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "approve", agg_id, com_approve).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "limit", agg_id, com_limit).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn stop_account_approved_false(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let false_approve = false_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let (_, agg_id, _) = create(app, PATH, "create", com_create).await;
            change(app, PATH, "verify", agg_id, true_verify).await;
            change(app, PATH, "approve", agg_id, false_approve).await;

            let (s, _) = change(app, PATH, "verify", agg_id, com_verify).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "approve", agg_id, com_approve).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "limit", agg_id, com_limit).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_account_created(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let (_, agg_id, _) = create(app, PATH, "create", com_create).await;

            let (s, _) = change(app, PATH, "approve", agg_id, com_approve).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "limit", agg_id, com_limit).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_account_verified_true(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let (_, agg_id, _) = create(app, PATH, "create", com_create).await;
            change(app, PATH, "verify", agg_id, true_verify).await;

            let (s, _) = change(app, PATH, "verify", agg_id, com_verify).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "limit", agg_id, com_limit).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_account_approved_true(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let true_approve = true_approve().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let (_, agg_id, _) = create(app, PATH, "create", com_create).await;
            change(app, PATH, "verify", agg_id, true_verify).await;
            change(app, PATH, "approve", agg_id, true_approve).await;

            let (s, _) = change(app, PATH, "verify", agg_id, com_verify).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
            let (s, _) = change(app, PATH, "approve", agg_id, com_approve).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn limit_account_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let true_approve = true_approve().new_tree(&mut runner).unwrap().current();
        let mut com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            com_limit.limit = true_approve.limit;
            let (_, agg_id, _) = create(app, PATH, "create", com_create).await;
            change(app, PATH, "verify", agg_id, true_verify).await;
            change(app, PATH, "approve", agg_id, true_approve).await;

            let (s, _) = change(app, PATH, "limit", agg_id, com_limit).await;
            prop_assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

            Ok(())
        });
    }

    if let Err(e) = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<()>, TestCaseError>>()
    {
        panic!("{e}");
    }

    ctx.teardown().await;
}
