use super::*;

#[rstest]
#[tokio::test]
async fn create_account(#[future(awt)] app: Router, #[future(awt)] ctx: &'static App) {
    let com = CreateAccount {
        code: "123456".to_string(),
        owner: "张三".to_string(),
    };
    let agg_id = Uuid::new_v4();
    let com_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::post(&route("create", agg_id, com_id))
                .header("Content-Type", "application/octet-stream")
                .body(Body::from(rkyv::to_bytes::<Error>(&com).unwrap().to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 64).await.unwrap();
    assert!(body.is_empty());

    ctx.teardown().await;
}
