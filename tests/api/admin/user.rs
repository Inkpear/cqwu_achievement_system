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
async fn create_user_user_success() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let body = serde_json::json!({
        "username": Uuid::new_v4().to_string(),
        "nickname": Uuid::new_v4().to_string(),
        "password": Uuid::new_v4().to_string()
    });

    let response = app
        .post_create_user(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to create_user user");

    check_response_code_and_message(&response, 201, "创建用户成功");

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
async fn create_user_user_is_rejected_when_username_already_exists() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let body = serde_json::json!({
        "username": "duplicate_user",
        "nickname": "Test User",
        "password": "password123"
    });

    let response = app
        .post_create_user(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to create_user user");

    check_response_code_and_message(&response, 201, "创建用户成功");

    let duplicate_body = serde_json::json!({
        "username": "duplicate_user",
        "nickname": "Another User",
        "password": "different_password"
    });

    let response = app
        .post_create_user(&duplicate_body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 409, "用户已经存在，请勿重复注册");
}

#[tokio::test]
async fn create_user_user_is_rejected_when_body_is_invalid() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let missing_field_cases = vec![
        serde_json::json!({}),
        serde_json::json!({"username": "test"}),
        serde_json::json!({"username": "test", "password": "test"}),
    ];

    for body in missing_field_cases {
        let response = app.post_create_user(&body).await;

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

    for (body, _) in validation_cases {
        let response = app
            .post_create_user(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&response, 400, "参数校验失败");
    }
}

#[tokio::test]
pub async fn admin_disable_user_and_user_cannot_operate() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "is_active": false
    });

    let response = app
        .patch_modify_user_status(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "修改用户状态成功");

    app.login(&user).await;

    let body = serde_json::json!({
        "raw_password": user.password,
        "new_password": "new_secure_password"
    });

    let response = app
        .patch_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 403, "账户已被禁用，请联系管理员");
}

#[tokio::test]
async fn admin_disable_user_fail_when_user_not_exists() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let body = serde_json::json!({
        "user_id": Uuid::new_v4().to_string(),
        "is_active": false
    });

    let response = app
        .patch_modify_user_status(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 404, "用户不存在");
}

#[tokio::test]
async fn create_user_and_filter_user_query_success() {
    let mut app = TestApp::spawn().await;
    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user_ids = vec![];
    for _ in 0..21 {
        let mut user = TestUser::new();
        user.store(&app.db_pool).await;
        assert!(user.user_id.is_some());
        user_ids.push(user.user_id.unwrap());
    }

    let query_params = serde_json::json!({
        "role": "USER",
        "page": 1,
        "page_size": 12
    });

    let response = app
        .get_query_user(&query_params)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "查询用户成功");

    let data = &response["data"];
    let users = data["items"].as_array().expect("Data items is not an array");
    assert_eq!(users.len(), 12);
    assert_eq!(data["total"].as_i64().unwrap(), 21);
    assert_eq!(data["total_pages"].as_i64().unwrap(), 2);
    assert_eq!(data["page"].as_i64().unwrap(), 1);

    for user in users.iter() {
        let uid = serde_json::from_value::<Uuid>(user["user_id"].clone())
            .expect("Failed to parse user_id");
        assert!(user_ids.contains(&uid));
    }
}

#[tokio::test]
async fn admin_change_user_password_and_user_login_success_with_new_password() {
    let mut app = TestApp::spawn().await;
    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "new_password": "new_secure_password"
    });

    let response = app
        .patch_change_user_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "修改用户密码成功");

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