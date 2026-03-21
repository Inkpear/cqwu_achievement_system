use crate::helper::TestApp;

#[tokio::test]
async fn health_check_works() {
    let app = TestApp::spawn().await;

    let response = app
        .api_client
        .get(format!("{}/health_check", &app.address))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
}
