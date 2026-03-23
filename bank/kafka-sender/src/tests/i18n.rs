use super::*;
use axum::http::header::{ACCEPT_LANGUAGE, CONTENT_TYPE};
use domain::account::*;
use unis_kafka::sender::KafkaSender;

#[fixture]
async fn app(_setup: (), ctx: &'static Context) -> Router {
    let svc = Arc::new(ctx.setup::<_, KafkaSender<AccountCommand>>().await);
    Router::new().merge(routes::account_routes().with_state(svc))
}

#[rstest]
#[tokio::test]
async fn invalid_length_zh(#[future(awt)] app: Router, ctx: &'static Context) {
    let com = CreateAccount {
        code: "12345".to_string(),
        owner: "张三".to_string(),
    };
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&create("/api/v1/rkyv/account", "create", com_id))
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(ACCEPT_LANGUAGE, "zh-CN")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert_eq!(body, Bytes::from("code: 长度应为 6"));

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn invalid_ascii_zh(#[future(awt)] app: Router, ctx: &'static Context) {
    let com = CreateAccount {
        code: "12345a".to_string(),
        owner: "张三".to_string(),
    };
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&create("/api/v1/rkyv/account", "create", com_id))
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(ACCEPT_LANGUAGE, "zh-CN")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert_eq!(body, Bytes::from("code: 应为 ASCII 数字"));

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn valid_zh(#[future(awt)] app: Router, ctx: &'static Context) {
    let com = CreateAccount {
        code: "123456".to_string(),
        owner: "张三".to_string(),
    };
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&create("/api/v1/rkyv/account", "create", com_id))
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(ACCEPT_LANGUAGE, "zh-CN")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert_eq!(body, Bytes::from("成功"));

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn invalid_length_en(#[future(awt)] app: Router, ctx: &'static Context) {
    let com = CreateAccount {
        code: "12345".to_string(),
        owner: "张三".to_string(),
    };
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&create("/api/v1/rkyv/account", "create", com_id))
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(ACCEPT_LANGUAGE, "en-US")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert_eq!(body, Bytes::from("code: length should be 6"));

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn invalid_ascii_en(#[future(awt)] app: Router, ctx: &'static Context) {
    let com = CreateAccount {
        code: "12345a".to_string(),
        owner: "张三".to_string(),
    };
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&create("/api/v1/rkyv/account", "create", com_id))
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(ACCEPT_LANGUAGE, "en-US")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert_eq!(body, Bytes::from("code: should be ASCII digit"));

    ctx.teardown().await;
}

#[rstest]
#[tokio::test]
async fn valid_en(#[future(awt)] app: Router, ctx: &'static Context) {
    let com = CreateAccount {
        code: "123456".to_string(),
        owner: "张三".to_string(),
    };
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&create("/api/v1/rkyv/account", "create", com_id))
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(ACCEPT_LANGUAGE, "en-US")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert_eq!(body, Bytes::from("success"));

    ctx.teardown().await;
}
