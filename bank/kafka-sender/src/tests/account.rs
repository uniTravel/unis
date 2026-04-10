use super::*;
use domain::account::*;

#[fixture]
async fn app(_setup: (), ctx: &'static Context) -> Router {
    let svc = Arc::new(ctx.setup::<_, KafkaSender<AccountCommand>>().await);
    Router::new().merge(routes::account_routes().with_state(svc))
}

const COUNT: usize = 7;
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

#[rstest]
#[tokio::test]
async fn start_account(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let agg_id = Uuid::new_v4();
            let app = app_c.clone();
            let res = change(app, PATH, "verify", agg_id, Uuid::new_v4(), com_verify).await?;
            prop_assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let app = app_c.clone();
            let res = change(app, PATH, "approve", agg_id, Uuid::new_v4(), com_approve).await?;
            prop_assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let app = app_c.clone();
            let res = change(app, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

            let res = create(app_c, PATH, "create", Uuid::new_v4(), com_create).await?;
            prop_assert_eq!(res.status(), StatusCode::OK);

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
async fn stop_account_verified_false(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let false_verify = false_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let app = app_c.clone();
            let res = create(app, PATH, "create", Uuid::new_v4(), com_create).await?;
            let body = to_bytes(res.into_body(), 36).await.unwrap();
            let agg_id = Uuid::parse_str(str::from_utf8(&body).unwrap()).unwrap();
            let app = app_c.clone();
            let _ = change(app, PATH, "verify", agg_id, Uuid::new_v4(), false_verify).await?;

            let app = app_c.clone();
            let res = change(app, PATH, "verify", agg_id, Uuid::new_v4(), com_verify).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let app = app_c.clone();
            let res = change(app, PATH, "approve", agg_id, Uuid::new_v4(), com_approve).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let res = change(app_c, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);

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
async fn stop_account_approved_false(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let false_approve = false_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let app = app_c.clone();
            let res = create(app, PATH, "create", Uuid::new_v4(), com_create).await?;
            let body = to_bytes(res.into_body(), 36).await.unwrap();
            let agg_id = Uuid::parse_str(str::from_utf8(&body).unwrap()).unwrap();
            let app = app_c.clone();
            let _ = change(app, PATH, "verify", agg_id, Uuid::new_v4(), true_verify).await?;
            let app = app_c.clone();
            let _ = change(app, PATH, "approve", agg_id, Uuid::new_v4(), false_approve).await?;

            let app = app_c.clone();
            let res = change(app, PATH, "verify", agg_id, Uuid::new_v4(), com_verify).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let app = app_c.clone();
            let res = change(app, PATH, "approve", agg_id, Uuid::new_v4(), com_approve).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let res = change(app_c, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);

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
async fn state_account_created(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let app = app_c.clone();
            let res = create(app, PATH, "create", Uuid::new_v4(), com_create).await?;
            let body = to_bytes(res.into_body(), 36).await.unwrap();
            let agg_id = Uuid::parse_str(str::from_utf8(&body).unwrap()).unwrap();

            let app = app_c.clone();
            let res = change(app, PATH, "approve", agg_id, Uuid::new_v4(), com_approve).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let app = app_c.clone();
            let res = change(app, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);

            let res = change(app_c, PATH, "verify", agg_id, Uuid::new_v4(), com_verify).await?;
            prop_assert_eq!(res.status(), StatusCode::OK);

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
async fn state_account_verified_true(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let app = app_c.clone();
            let res = create(app, PATH, "create", Uuid::new_v4(), com_create).await?;
            let body = to_bytes(res.into_body(), 36).await.unwrap();
            let agg_id = Uuid::parse_str(str::from_utf8(&body).unwrap()).unwrap();
            let app = app_c.clone();
            let _ = change(app, PATH, "verify", agg_id, Uuid::new_v4(), true_verify).await?;

            let app = app_c.clone();
            let res = change(app, PATH, "verify", agg_id, Uuid::new_v4(), com_verify).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let app = app_c.clone();
            let res = change(app, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);

            let res = change(app_c, PATH, "approve", agg_id, Uuid::new_v4(), com_approve).await?;
            prop_assert_eq!(res.status(), StatusCode::OK);

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
async fn state_account_approved_true(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let com_verify = valid_verify().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let com_approve = valid_approve().new_tree(&mut runner).unwrap().current();
        let true_approve = true_approve().new_tree(&mut runner).unwrap().current();
        let com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            let app = app_c.clone();
            let res = create(app, PATH, "create", Uuid::new_v4(), com_create).await?;
            let body = to_bytes(res.into_body(), 36).await.unwrap();
            let agg_id = Uuid::parse_str(str::from_utf8(&body).unwrap()).unwrap();
            let app = app_c.clone();
            let _ = change(app, PATH, "verify", agg_id, Uuid::new_v4(), true_verify).await?;
            let app = app_c.clone();
            let _ = change(app, PATH, "approve", agg_id, Uuid::new_v4(), true_approve).await?;

            let app = app_c.clone();
            let res = change(app, PATH, "verify", agg_id, Uuid::new_v4(), com_verify).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);
            let app = app_c.clone();
            let res = change(app, PATH, "approve", agg_id, Uuid::new_v4(), com_approve).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);

            let res = change(app_c, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::OK);

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
async fn limit_account_restrict(#[future(awt)] app: Router, ctx: &'static Context) {
    let mut runner = TestRunner::default();
    let mut tasks = JoinSet::new();

    for _ in 0..COUNT {
        let app_c = app.clone();
        let com_create = valid_create().new_tree(&mut runner).unwrap().current();
        let true_verify = true_verify().new_tree(&mut runner).unwrap().current();
        let true_approve = true_approve().new_tree(&mut runner).unwrap().current();
        let mut com_limit = valid_limit().new_tree(&mut runner).unwrap().current();
        tasks.spawn(async move {
            com_limit.limit = true_approve.limit;
            let app = app_c.clone();
            let res = create(app, PATH, "create", Uuid::new_v4(), com_create).await?;
            let body = to_bytes(res.into_body(), 36).await.unwrap();
            let agg_id = Uuid::parse_str(str::from_utf8(&body).unwrap()).unwrap();
            let app = app_c.clone();
            let _ = change(app, PATH, "verify", agg_id, Uuid::new_v4(), true_verify).await?;
            let app = app_c.clone();
            let _ = change(app, PATH, "approve", agg_id, Uuid::new_v4(), true_approve).await?;

            let res = change(app_c, PATH, "limit", agg_id, Uuid::new_v4(), com_limit).await?;
            prop_assert_eq!(res.status(), StatusCode::BAD_REQUEST);

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
