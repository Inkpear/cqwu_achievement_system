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
async fn register_user_is_rejected_when_body_is_invalid() {
    let app = TestApp::spawn().await;
    
    let missing_field_cases = vec![
        serde_json::json!({}),
        serde_json::json!({"username": "test"}),
        serde_json::json!({"username": "test", "password": "test"}),
    ];

    for body in missing_field_cases {
        let response = app
            .post_register(&body)
            .await;
        
        assert_eq!(response.status().as_u16(), 400);
    }

    let validation_cases = vec![
        (serde_json::json!({"username": "", "nickname": "nick", "password": "pass"}), "用户名必须在3-50个字符之间"),
        (serde_json::json!({"username": "ab", "nickname": "nick", "password": "password"}), "用户名必须在3-50个字符之间"),
        (serde_json::json!({"username": "test", "nickname": "", "password": "password"}), "昵称必须在3-50个字符之间"),
        (serde_json::json!({"username": "test", "nickname": "ni", "password": "password"}), "昵称必须在3-50个字符之间"),
        (serde_json::json!({"username": "test", "nickname": "nick", "password": ""}), "密码必须在6-100个字符之间"),
        (serde_json::json!({"username": "test", "nickname": "nick", "password": "12345"}), "密码必须在6-100个字符之间"),
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
