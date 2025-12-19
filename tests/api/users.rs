use uuid::Uuid;

use crate::helper::{TestApp, TestUser, check_response_code_and_message};

#[tokio::test]
async fn create_user_persists_to_database() {
    let app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let row = sqlx::query!(
        r#"
            SELECT username, nickname FROM sys_user
            WHERE user_id = $1
        "#,
        user.user_id
    )
    .fetch_one(&app.db_pool)
    .await
    .expect("Failed to fetch saved user");

    assert_eq!(row.username, user.username);
    assert_eq!(row.nickname, user.nickname);
}

#[tokio::test]
async fn register_user_success() {
    let app = TestApp::spawn().await;

    let body = serde_json::json!({
        "username": Uuid::new_v4().to_string(),
        "nickname": Uuid::new_v4().to_string(),
        "password": Uuid::new_v4().to_string()
    });

    let response = app
        .post_register(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to register user");

    check_response_code_and_message(&response, 201, "注册成功");

    let response_body = &response["data"];

    assert!(response_body.get("user_id").is_some());
    assert_eq!(
        response_body.get("username").unwrap().as_str(),
        body["username"].as_str()
    );
    assert_eq!(
        response_body.get("nickname").unwrap().as_str(),
        body["nickname"].as_str()
    );
}

#[tokio::test]
async fn register_user_is_rejected_when_username_already_exists() {
    let app = TestApp::spawn().await;

    let body = serde_json::json!({
        "username": "duplicate_user",
        "nickname": "Test User",
        "password": "password123"
    });

    let response = app
        .post_register(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to register user");

    check_response_code_and_message(&response, 201, "注册成功");

    let duplicate_body = serde_json::json!({
        "username": "duplicate_user",
        "nickname": "Another User",
        "password": "different_password"
    });

    let response = app
        .post_register(&duplicate_body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 409, "用户已经存在，请勿重复注册");
}

#[tokio::test]
async fn register_user_is_rejected_when_body_is_invalid() {
    let app = TestApp::spawn().await;

    let missing_field_cases = vec![
        serde_json::json!({}),
        serde_json::json!({"username": "test"}),
        serde_json::json!({"username": "test", "password": "test"}),
    ];

    for body in missing_field_cases {
        let response = app.post_register(&body).await;

        assert_eq!(response.status().as_u16(), 400);
    }

    let validation_cases = vec![
        (
            serde_json::json!({"username": "", "nickname": "nick", "password": "pass"}),
            "用户名必须在3-50个字符之间",
        ),
        (
            serde_json::json!({"username": "ab", "nickname": "nick", "password": "password"}),
            "用户名必须在3-50个字符之间",
        ),
        (
            serde_json::json!({"username": "test", "nickname": "", "password": "password"}),
            "昵称必须在3-50个字符之间",
        ),
        (
            serde_json::json!({"username": "test", "nickname": "ni", "password": "password"}),
            "昵称必须在3-50个字符之间",
        ),
        (
            serde_json::json!({"username": "test", "nickname": "nick", "password": ""}),
            "密码必须在6-100个字符之间",
        ),
        (
            serde_json::json!({"username": "test", "nickname": "nick", "password": "12345"}),
            "密码必须在6-100个字符之间",
        ),
    ];

    for (body, expected_message) in validation_cases {
        let response = app
            .post_register(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&response, 400, expected_message);
    }
}

#[tokio::test]
async fn login_success_and_recieved_a_valid_jwt() {
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
        (
            serde_json::json!({"username": "123", "password": ""}),
            "密码不能为空".into(),
        ),
        (
            serde_json::json!({"password": "123", "username": ""}),
            "用户名不能为空".into(),
        ),
    ];

    for (body, expected_message) in test_cases {
        let response = app
            .post_login(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&response, 400, expected_message);
    }
}

#[tokio::test]
async fn change_passwrod_is_rejected_when_old_password_is_wrong() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "raw_password": "wrong_old_password",
        "new_password": "new_secure_password"
    });

    let response = app
        .post_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 403, "密码错误，请检查您的输入是否正确");
}

#[tokio::test]
async fn change_passwrod_success() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "raw_password": user.password,
        "new_password": "new_secure_password"
    });

    let response = app
        .post_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "修改密码成功");

    let login_body = serde_json::json!({
        "username": user.username,
        "password": "new_secure_password"
    });

    let response = app
        .post_login(&login_body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "登录成功");
}

#[tokio::test]
async fn try_to_change_password_failed_when_not_logged_in() {
    let app = TestApp::spawn().await;

    let body = serde_json::json!({
        "raw_password": "any_password",
        "new_password": "new_secure_password"
    });

    let response = app
        .post_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 401, "未授权访问，请先登录");
}

#[tokio::test]
async fn when_user_is_disabled_request_is_rejected() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

    sqlx::query!(
        r#"
        UPDATE sys_user
        SET is_active = FALSE
        WHERE user_id = $1
        "#,
        user.user_id
    )
    .execute(&app.db_pool)
    .await
    .expect("Failed to disable user");

    app.login(&user).await;

    let body = serde_json::json!({
        "raw_password": user.password,
        "new_password": "new_secure_password"
    });

    let response = app
        .post_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 401, "用户已被禁用，请联系管理员");
}
