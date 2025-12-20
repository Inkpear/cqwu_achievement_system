use uuid::Uuid;

use crate::helper::{TestApp, TestUser, check_response_code_and_message};

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
