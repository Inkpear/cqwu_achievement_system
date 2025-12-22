use crate::helper::{TestApp, TestUser, check_response_code_and_message};

#[tokio::test]
async fn login_success_and_received_a_valid_jwt() {
    let app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

    let body = serde_json::json!({
        "username": user.username,
        "password": user.password
    });

    let response = app
        .post_login(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "登录成功");

    assert!(response.get("data").is_some());

    let data = response.get("data").unwrap();
    let jwt = data.get("token").unwrap().as_str().unwrap();

    let claims = app
        .jwt_config
        .verify_jwt_token(jwt)
        .expect("JWT verification failed");

    assert_eq!(claims.sub, user.user_id.unwrap());
    assert_eq!(claims.username, user.username);
}

#[tokio::test]
async fn login_is_rejected_with_invalid_credentials() {
    let app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

    let body = serde_json::json!({
        "username": user.username,
        "password": "wrong_password"
    });

    let response = app
        .post_login(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 401, "登录失败，请检查用户名或密码是否正确");

    let body = serde_json::json!({
        "username": "wrong_username",
        "password": "wrong_password"
    });

    let response = app
        .post_login(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 401, "登录失败，请检查用户名或密码是否正确");
}

#[tokio::test]
async fn login_is_rejected_with_missing_credentials() {
    let app = TestApp::spawn().await;

    let test_cases = vec![
        serde_json::json!({}),
        serde_json::json!({"username": ""}),
        serde_json::json!({"password": ""}),
    ];

    for body in test_cases {
        let response = app.post_login(&body).await;

        assert_eq!(response.status().as_u16(), 400);
    }

    let test_cases = vec![
        serde_json::json!({"username": "123", "password": ""}),
        serde_json::json!({"password": "123", "username": ""}),
    ];

    for body in test_cases {
        let response = app
            .post_login(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&response, 400, "参数校验失败");
    }
}
