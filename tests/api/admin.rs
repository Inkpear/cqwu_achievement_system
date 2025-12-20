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

    for (body, expected_message) in validation_cases {
        let response = app
            .post_create_user(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&response, 400, expected_message);
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
        .put_change_password(&body)
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
async fn admin_grant_user_create_user_api_rule_and_user_can_create_user() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/admin/user/create/",
        "http_method": "POST",
        "description": "允许访问管理员用户接口",
        "expires_at": null
    });

    let response = app
        .post_grant_user_api_rule(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");

    app.login(&user).await;

    let create_user_body = serde_json::json!({
        "username": Uuid::new_v4().to_string(),
        "nickname": Uuid::new_v4().to_string(),
        "password": Uuid::new_v4().to_string()
    });

    let response = app
        .post_create_user(&create_user_body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "创建用户成功");
}

#[tokio::test]
async fn grant_api_rule_fail_when_rule_conflict() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/user/",
        "http_method": "ALL",
        "expires_at": null
    });

    let response = app
        .post_grant_user_api_rule(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");

    let rule_id = response["data"].get("rule_id");
    assert!(rule_id.is_some());
    let rule_id = rule_id.unwrap();

    let conflicting_body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/user/profile/",
        "http_method": "GET",
        "expires_at": null
    });

    let response = app
        .post_grant_user_api_rule(&conflicting_body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 409, "存在更宽泛的API访问规则");

    let conflicting_rule_id = response["message"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .last()
        .unwrap();
    assert_eq!(conflicting_rule_id, rule_id.as_str().unwrap());
}

#[tokio::test]
async fn grant_same_rule_success_when_expires_is_longer() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/user/profile/",
        "http_method": "GET",
        "expires_at": "2026-12-31T23:59:59Z"
    });

    let response = app
        .post_grant_user_api_rule(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");

    let rule_id = response["data"].get("rule_id");
    assert!(rule_id.is_some());
    let rule_id = rule_id.unwrap();

    let extended_body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/user/profile/",
        "http_method": "GET",
        "expires_at": "2027-12-31T23:59:59Z"
    });

    let response = app
        .post_grant_user_api_rule(&extended_body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");

    let extended_rule_id = response["data"].get("rule_id");
    assert!(extended_rule_id.is_some());
    let extended_rule_id = extended_rule_id.unwrap();

    assert_eq!(
        rule_id.as_str().unwrap(),
        extended_rule_id.as_str().unwrap()
    );
}

#[tokio::test]
async fn grant_api_rule_fail_when_invalid_body() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let invalid_bodies = vec![
        serde_json::json!({}),
        serde_json::json!({"user_id": Uuid::new_v4().to_string()}),
        serde_json::json!({
            "user_id": Uuid::new_v4().to_string(),
            "api_pattern": "/api/test"
        }),
    ];

    for body in invalid_bodies {
        let response = app.post_grant_user_api_rule(&body).await;

        assert_eq!(response.status().as_u16(), 400);
    }
}

#[tokio::test]
async fn revoke_exists_rule_success_and_not_exists_returns_404() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/user/profile/",
        "http_method": "GET",
        "expires_at": null
    });

    let response = app
        .post_grant_user_api_rule(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");

    let rule_id = response["data"].get("rule_id").unwrap().as_str().unwrap();

    let response = app
        .delete_revoke_user_api_rule(rule_id)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "撤销用户 API 访问规则成功");

    let response = app
        .delete_revoke_user_api_rule(rule_id)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 404, "API访问规则不存在");
}

#[tokio::test]
async fn grant_api_rule_and_query_success() {
    let mut app = TestApp::spawn().await;

    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;

    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    assert!(user.user_id.is_some());

    for i in 1..=15 {
        let body = serde_json::json!({
            "user_id": user.user_id.unwrap().to_string(),
            "api_pattern": format!("/api/test/endpoint_{}/", ('a' as u8 + i) as char),
            "http_method": "GET",
            "expires_at": null
        });

        let response = app
            .post_grant_user_api_rule(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");
    }

    let user_id = user.user_id.unwrap();
    let page = 2;
    let page_size = 5;

    let response = app
        .get_query_user_api_rules(Some(&user_id.to_string()), page, page_size)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "查询用户 API 访问规则成功");

    let data = &response["data"];
    assert_eq!(data["total"].as_i64().unwrap(), 15);
    assert_eq!(data["page"].as_i64().unwrap(), 2);
    assert_eq!(data["page_size"].as_i64().unwrap(), 5);
    assert_eq!(data["items"].as_array().unwrap().len(), 5);
}
