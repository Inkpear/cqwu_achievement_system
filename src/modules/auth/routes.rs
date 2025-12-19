use crate::modules::auth::service::{generate_jwt, validate_user_password};
use actix_web::{Responder, web};

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    modules::auth::model::{LoginForm, LoginRequest, LoginResponse},
};

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/auth/login",
        tag = "用户认证",
        request_body(
            content = LoginRequest,
            content_type = "application/x-www-form-urlencoded"
        ),
        responses(
            (status = 200, description = "登录成功", body = AppResponse<LoginResponse>),
            (status = 400, description = "参数校验失败"),
            (status = 401, description = "登录失败，请检查用户名或密码是否正确")
        )
    )
)]
#[tracing::instrument(name = "登录用户", skip(req, app_state), fields(user_id = tracing::field::Empty))]
pub async fn login_user_handler(
    app_state: web::Data<AppState>,
    req: web::Form<LoginRequest>,
) -> Result<impl Responder, AppError> {
    let login_form =
        LoginForm::try_from_request(req.0).map_err(|e| AppError::ValidationError(e.to_string()))?;

    let (user_id, role) =
        validate_user_password(&login_form.username, login_form.password, &app_state.pool)
            .await
            .map_err(|_| AppError::LoginFailed)?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id));

    let jwt = generate_jwt(&app_state.jwt_config, user_id, &login_form.username, role).await?;

    let response = LoginResponse { token: jwt };

    Ok(AppResponse::success_msg(response, "登录成功"))
}
