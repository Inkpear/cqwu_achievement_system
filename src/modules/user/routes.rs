use actix_web::{Responder, web};

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::{
        auth::service::validate_user_password,
        user::{
            models::{ChangePassword, ChangePasswordRequest},
            service::change_user_password,
        },
    },
};

#[cfg(feature = "swagger")]
use crate::common::response::EmptyData;

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        patch,
        path = "/api/user/password",
        tag = "用户管理",
        security(
            ("bearer_auth" = [])
        ),
        request_body = ChangePasswordRequest,
        responses(
            (status = 200, description = "修改密码成功", body = AppResponse<EmptyData>),
            (status = 400, description = "参数校验失败"),
            (status = 403, description = "密码错误，请检查您的输入是否正确")
        )
    )
)]
#[tracing::instrument(
    name = "用户修改密码",
    skip(app_state, req, claims),
    fields(
        user_id = %claims.sub,
        username = %claims.username
    )
)]
pub async fn change_password_handler(
    app_state: web::Data<AppState>,
    req: web::Json<ChangePasswordRequest>,
    claims: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let change_password_body = ChangePassword::try_from_request(req.0)
        .map_err(AppError::ValidationError)?;

    validate_user_password(
        &claims.username,
        change_password_body.raw_password,
        &app_state.pool,
    )
    .await
    .map_err(|_| AppError::PasswordWrong)?;

    change_user_password(
        &app_state.pool,
        claims.sub,
        change_password_body.new_password,
    )
    .await?;

    Ok(AppResponse::ok_msg("修改密码成功"))
}
