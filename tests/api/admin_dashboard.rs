use crate::helper::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn you_must_be_logged_in_to_access_the_admin_dashboard() {
    let app = spawn_app().await;
    let response = app.get_admin_dashboard().await;
    assert_is_redirect_to(&response, "/login");
}

#[tokio::test]
async fn logout_clears_the_session_state() {
    let app = spawn_app().await;

    let response = app
        .post_login(&serde_json::json!({
            "username": &app.test_user.username,
            "password": &app.test_user.password
        }))
        .await;

    assert_is_redirect_to(&response, "/admin/dashboard");

    let response = app.post_logout().await;
    assert_is_redirect_to(&response, "/login");

    let response = app.get_admin_dashboard().await;
    assert_is_redirect_to(&response, "/login");
}
