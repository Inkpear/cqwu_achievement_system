use crate::helper::{TestApp, TestUser, check_response_code_and_message};

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
        .put_change_password(&body)
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
        .put_change_password(&body)
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
        .put_change_password(&body)
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
        .put_change_password(&body)
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
        .put_change_password(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "修改密码成功");
}
