use actix_web::{Responder, web};

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    modules::admin::{
        models::{RegisterUser, RegisterUserRequest, UserResponse},
        service::store_user,
    },
    utils::password::hash_password,
};

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/admin/create_user",
        tag = "管理员操作",
        request_body = RegisterUserRequest,
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 201, description = "创建用户成功", body = AppResponse<UserResponse>),
            (status = 400, description = "参数校验失败"),
            (status = 409, description = "用户已存在")
        )
    )
)]
#[tracing::instrument(
    name = "创建用户",
    skip(app_state, req),
    fields(
        user_id = tracing::field::Empty,
        username = %req.username,
        nickname = %req.nickname
    )
)]
pub async fn create_user_handler(
    app_state: web::Data<AppState>,
    req: web::Json<RegisterUserRequest>,
) -> Result<impl Responder, AppError> {
    let mut user = RegisterUser::try_from_request(req.0)
        .map_err(|e| AppError::ValidationError(e.to_string()))?;

    user.password = hash_password(user.password)
        .await
        .map_err(AppError::UnexpectedError)?;

    let user_id = store_user(&app_state.pool, &user).await?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id));

    let response = UserResponse {
        user_id,
        username: user.username,
        nickname: user.nickname,
    };

    Ok(AppResponse::created(response, "创建用户成功"))
}
