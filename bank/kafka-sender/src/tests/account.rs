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

const PATH: &str = "/api/v1/rkyv/account";

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

#[derive(Clone, Debug)]
struct RefAccount {
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
                create().prop_map(AccountCommand::Create).boxed()
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
                verify().prop_map(AccountCommand::Verify).boxed()
            }
            RefAccount {
                code: _,
                owner: _,
                limit: 0,
                verified_by: _,
                verified: _,
                approved_by,
                approved: false,
            } if approved_by.is_empty() => approve().prop_map(AccountCommand::Approve).boxed(),
            RefAccount {
                code: _,
                owner: _,
                limit: _,
                verified_by: _,
                verified: true,
                approved_by,
                approved: _,
            } if !approved_by.is_empty() => limit().prop_map(AccountCommand::Limit).boxed(),
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
        match transition {
            AccountCommand::Limit(com) => com.limit != state.limit,
            _ => true,
        }
    }
}

fn process(body: Bytes, mut state: Account) -> Account {
    match AccountEvent::from_bytes(&body).unwrap() {
        AccountEvent::Created(evt) => evt.process(&mut state),
        AccountEvent::Verified(evt) => evt.process(&mut state),
        AccountEvent::Approved(evt) => evt.process(&mut state),
        AccountEvent::Limited(evt) => evt.process(&mut state),
    }
    state
}

#[rstest]
#[tokio::test]
async fn account_state_machine(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let (mut ref_state, transitions, _) = RefAccount::sequential_strategy(2..6)
        .new_tree(&mut TestRunner::default())
        .unwrap()
        .current();
    let mut state = Account::new(Uuid::new_v4());

    for transition in transitions {
        ref_state = RefAccount::apply(ref_state, &transition);
        match transition {
            AccountCommand::Create(com) => {
                let agg_id = Uuid::new_v4();
                let (_, body) = apply(app, PATH, "create", agg_id, com).await;
                state = process(body, Account::new(agg_id));
            }
            AccountCommand::Verify(com) => {
                let (_, body) = apply(app, PATH, "verify", state.id(), com).await;
                state = process(body, state);
            }
            AccountCommand::Approve(com) => {
                if ref_state.verified {
                    let (_, body) = apply(app, PATH, "approve", state.id(), com).await;
                    state = process(body, state);
                }
            }
            AccountCommand::Limit(com) => {
                if ref_state.approved {
                    let (_, body) = apply(app, PATH, "limit", state.id(), com).await;
                    state = process(body, state);
                }
            }
        }
    }

    assert_eq!(ref_state.code, state.code);
    assert_eq!(ref_state.owner, state.owner);
    assert_eq!(ref_state.verified_by, state.verified_by);
    assert_eq!(ref_state.verified, state.verified);
    assert_eq!(ref_state.approved_by, state.approved_by);
    assert_eq!(ref_state.approved, state.approved);
    assert_eq!(ref_state.limit, state.limit);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn stop_account_verified_false(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let create = create().new_tree(&mut runner).unwrap().current();
    let verify = verify().new_tree(&mut runner).unwrap().current();
    let false_verify = false_verify().new_tree(&mut runner).unwrap().current();
    let approve = approve().new_tree(&mut runner).unwrap().current();
    let limit = limit().new_tree(&mut runner).unwrap().current();
    let agg_id = Uuid::new_v4();
    apply(app, PATH, "create", agg_id, create).await;
    apply(app, PATH, "verify", agg_id, false_verify).await;

    let (s, _) = apply(app, PATH, "verify", agg_id, verify).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "approve", agg_id, approve).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "limit", agg_id, limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn stop_account_approved_false(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let create = create().new_tree(&mut runner).unwrap().current();
    let verify = verify().new_tree(&mut runner).unwrap().current();
    let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
    let approve = approve().new_tree(&mut runner).unwrap().current();
    let false_approve = false_approve().new_tree(&mut runner).unwrap().current();
    let limit = limit().new_tree(&mut runner).unwrap().current();
    let agg_id = Uuid::new_v4();
    apply(app, PATH, "create", agg_id, create).await;
    apply(app, PATH, "verify", agg_id, true_verify).await;
    apply(app, PATH, "approve", agg_id, false_approve).await;

    let (s, _) = apply(app, PATH, "verify", agg_id, verify).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "approve", agg_id, approve).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "limit", agg_id, limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_account_created(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let create = create().new_tree(&mut runner).unwrap().current();
    let approve = approve().new_tree(&mut runner).unwrap().current();
    let limit = limit().new_tree(&mut runner).unwrap().current();
    let agg_id = Uuid::new_v4();
    apply(app, PATH, "create", agg_id, create).await;

    let (s, _) = apply(app, PATH, "approve", agg_id, approve).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "limit", agg_id, limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_account_verified_true(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let create = create().new_tree(&mut runner).unwrap().current();
    let verify = verify().new_tree(&mut runner).unwrap().current();
    let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
    let limit = limit().new_tree(&mut runner).unwrap().current();
    let agg_id = Uuid::new_v4();
    apply(app, PATH, "create", agg_id, create).await;
    apply(app, PATH, "verify", agg_id, true_verify).await;

    let (s, _) = apply(app, PATH, "verify", agg_id, verify).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "limit", agg_id, limit).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn state_account_approved_true(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let create = create().new_tree(&mut runner).unwrap().current();
    let verify = verify().new_tree(&mut runner).unwrap().current();
    let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
    let approve = approve().new_tree(&mut runner).unwrap().current();
    let true_approve = true_approve().new_tree(&mut runner).unwrap().current();
    let agg_id = Uuid::new_v4();
    apply(app, PATH, "create", agg_id, create).await;
    apply(app, PATH, "verify", agg_id, true_verify).await;
    apply(app, PATH, "approve", agg_id, true_approve).await;

    let (s, _) = apply(app, PATH, "verify", agg_id, verify).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    let (s, _) = apply(app, PATH, "approve", agg_id, approve).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn limit_account_restrict(#[future(awt)] app: &'static Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let create = create().new_tree(&mut runner).unwrap().current();
    let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
    let true_approve = true_approve().new_tree(&mut runner).unwrap().current();
    let agg_id = Uuid::new_v4();
    let (_, body) = apply(app, PATH, "create", agg_id, create).await;
    let mut state = Account::new(agg_id);
    state = process(body, state);
    let (_, body) = apply(app, PATH, "verify", agg_id, true_verify).await;
    state = process(body, state);
    let (_, body) = apply(app, PATH, "approve", agg_id, true_approve).await;
    state = process(body, state);
    let mut com = limit().new_tree(&mut runner).unwrap().current();
    com.limit = state.limit;

    let (s, _) = apply(app, PATH, "limit", agg_id, com).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);

    ctx.teardown().await;
}
