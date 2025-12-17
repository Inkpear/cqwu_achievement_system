use crate::helper::{TestApp, TestUser};

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
