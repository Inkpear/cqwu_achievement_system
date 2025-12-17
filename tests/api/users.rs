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
