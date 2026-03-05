use crate::helper::{
    TestApp, TestUser, check_response_code_and_message, generate_a_dummy_file_content,
};

#[tokio::test]
async fn change_password_is_rejected_when_old_password_is_wrong() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "raw_password": "wrong_old_password",
        "new_password": "new_secure_password"
    });

    let response = app
        .patch_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 403, "密码错误，请检查您的输入是否正确");
}

#[tokio::test]
async fn change_password_success() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

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
        .patch_change_password(&body)
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
        .patch_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 403, "账户已被禁用，请联系管理员");
}

#[tokio::test]
pub async fn normal_user_has_basic_role_to_change_password() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

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

    check_response_code_and_message(&response, 200, "修改密码成功");
}

#[tokio::test]
pub async fn user_update_avatar_success() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    app.login(&user).await;

    let filename = "avatar.png";
    let content_length = 1024 * 1024;
    let dummy_file_content = generate_a_dummy_file_content(content_length);

    let body = serde_json::json!({
        "filename": filename,
        "content_length": content_length
    });

    let presigned_response = app
        .post_to_presigned_avatar_url(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&presigned_response, 201, "获取头像上传预签名 URL 成功");
    let file_id = presigned_response["data"]["file_id"]
        .as_str()
        .expect("file_id should be a string");
    let presigned_url = presigned_response["data"]["url"]
        .as_str()
        .expect("presigned_url should be a string");
    let upload_response = reqwest::Client::new()
        .put(presigned_url)
        .header("Content-Type", "image/png")
        .header("x-amz-meta-original-filename", filename)
        .body(dummy_file_content.clone())
        .send()
        .await
        .expect("Failed to upload avatar");

    assert!(upload_response.status().is_success());
    let update_response = app
        .patch_to_update_avatar(file_id)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    check_response_code_and_message(&update_response, 200, "更新用户头像成功");
    let view_url = update_response["data"]
        .as_str()
        .expect("avatar_url should be a string");

    let head_data = reqwest::Client::new()
        .get(view_url)
        .send()
        .await
        .expect("Failed to send GET request for avatar");

    let body_bytes = head_data
        .bytes()
        .await
        .expect("Failed to read avatar bytes");

    assert_eq!(body_bytes, dummy_file_content);
}

#[tokio::test]
async fn user_get_user_info_success() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    app.login(&user).await;

    let response = app
        .get_user_info()
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "获取个人信息成功");
    let data = &response["data"];
    assert_eq!(
        data["username"]
            .as_str()
            .expect("username should be a string"),
        user.username
    );
    assert_eq!(
        data["nickname"]
            .as_str()
            .expect("nickname should be a string"),
        user.nickname
    );
    assert_eq!(
        data["role"].as_str().expect("role should be a string"),
        "USER"
    );
}

#[tokio::test]
async fn user_update_user_info_success() {
    let mut app = TestApp::spawn().await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;
    app.login(&user).await;

    let new_email = "inkpear202413@gmail.com";
    let new_phone = "13002326950";
    let body = serde_json::json!({
        "email": new_email,
        "phone": new_phone
    });
    let response = app
        .patch_to_update_user_info(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    check_response_code_and_message(&response, 200, "更新用户信息成功");

    let user_info = app
        .get_user_info()
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    let data = &user_info["data"];

    assert_eq!(
        data["email"].as_str().expect("email should be a string"),
        new_email
    );
    assert_eq!(
        data["phone"].as_str().expect("phone should be a string"),
        new_phone
    );
}

#[tokio::test]
async fn grant_user_rule_and_user_query_effective_routes_success() {
    let mut app = TestApp::spawn().await;
    let admin = TestUser::default_admin(&app.db_pool).await;
    app.login(&admin).await;
    let mut user = TestUser::new();
    user.store(&app.db_pool).await;

    let body = serde_json::json!({
        "user_id": user.user_id.unwrap().to_string(),
        "api_pattern": "/api/",
        "http_method": "ALL",
        "expire_at": null,
        "description": "测试授予用户 API 访问权限"
    });

    let response = app
        .post_grant_user_api_rule(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    check_response_code_and_message(&response, 201, "授予用户 API 访问规则成功");

    app.login(&user).await;

    let response = app
        .get_user_effective_routes()
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    check_response_code_and_message(&response, 200, "获取用户有效路由成功");
    let data = response["data"]
        .as_array()
        .expect("data should be an array");

    assert!(!data.is_empty());
}
