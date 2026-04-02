use actix_web::{Responder, web};
use validator::Validate;

use crate::{
    common::{app_state::AppState, error::AppError, response::AppResponse},
    middleware::auth::AuthenticatedUser,
    modules::{
        auth::service::validate_user_password,
        user::{
            models::{
                ChangePassword, ChangePasswordRequest, PresignedAvatarUrlRequest,
                UpdateUserInfoRequest,
            },
            service::{
                build_avatar_key, change_user_password, get_user_effective_routes,
                get_user_info_by_id, parse_avatar_key_to_url, presigned_avatar_url,
                save_user_avatar_into_database, store_user_avatar, update_user_info,
            },
        },
    },
    utils::s3_storage::{build_avatar_dest_key, build_temp_avatar_key},
};

#[cfg(feature = "swagger")]
use {
    crate::domain::RouteInfo,
    crate::modules::user::models::{PresignedAvatarUrlResponse, UserInfoDTO},
};

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
            (status = 200, description = "修改密码成功"),
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
    let change_password_body =
        ChangePassword::try_from_request(req.0).map_err(AppError::ValidationError)?;

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

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        post,
        path = "/api/user/avatar/presigned",
        tag = "用户管理",
        security(
            ("bearer_auth" = [])
        ),
        request_body = PresignedAvatarUrlRequest,
        responses(
            (status = 201, description = "获取头像上传预签名 URL 成功", body = PresignedAvatarUrlResponse),
            (status = 400, description = "参数校验失败"),
        )
    )
)]
pub async fn presigned_avatar_url_handler(
    app_state: web::Data<AppState>,
    req: web::Json<PresignedAvatarUrlRequest>,
) -> Result<impl Responder, AppError> {
    let req = req.into_inner();
    req.validate().map_err(AppError::ValidationError)?;

    let content_type = req
        .content_type
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            mime_guess::from_path(&req.filename)
                .first_or_octet_stream()
                .essence_str()
                .to_string()
        });
    let presigned_response = presigned_avatar_url(
        &app_state.s3_storage,
        &req.filename,
        req.content_length,
        &content_type,
    )
    .await?;

    Ok(AppResponse::created(
        presigned_response,
        "获取头像上传预签名 URL 成功",
    ))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        patch,
        path = "/api/user/avatar/{file_id}",
        tag = "用户管理",
        params(
            ("file_id" = Uuid, Path, description = "头像文件 ID")
        ),
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "更新用户头像成功", body = String),
            (status = 400, description = "序列化失败"),
            (status = 404, description = "头像文件不存在"),
        )
    )
)]
pub async fn update_avatar_handler(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    file_id: web::Path<uuid::Uuid>,
) -> Result<impl Responder, AppError> {
    let source_key = build_temp_avatar_key(&file_id);
    let dest_key = build_avatar_dest_key(&user.sub);

    let file_metadata = store_user_avatar(&app_state.s3_storage, &source_key, &dest_key).await?;
    let avatar_key = build_avatar_key(&dest_key, &file_metadata.filename)?;
    save_user_avatar_into_database(&app_state.pool, &user.sub, &avatar_key).await?;
    let view_url = parse_avatar_key_to_url(&app_state.s3_storage, &avatar_key).await?;

    Ok(AppResponse::success_msg(view_url, "更新用户头像成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        patch,
        path = "/api/user/update",
        tag = "用户管理",
        security(
            ("bearer_auth" = [])
        ),
        request_body = UpdateUserInfoRequest,
        responses(
            (status = 200, description = "更新用户信息成功"),
            (status = 400, description = "参数校验失败"),
        )
    )
)]
#[tracing::instrument(
    name = "更新用户信息",
    skip(app_state, user, req),
    fields(
        user_id = %user.sub,
        username = %user.username
    )
)]
pub async fn update_user_info_handler(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    req: web::Json<UpdateUserInfoRequest>,
) -> Result<impl Responder, AppError> {
    let req = req.into_inner();
    req.validate().map_err(AppError::ValidationError)?;

    update_user_info(&app_state.pool, &user.sub, &req).await?;

    Ok(AppResponse::ok_msg("更新用户信息成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/user/me",
        tag = "用户管理",
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "获取个人信息成功", body = UserInfoDTO),
        )
    )
)]
#[tracing::instrument(name = "获取个人信息", skip(app_state, user))]
pub async fn get_user_info_handler(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let user_dto = get_user_info_by_id(&app_state.s3_storage, &user.sub, &app_state.pool).await?;

    Ok(AppResponse::success_msg(user_dto, "获取个人信息成功"))
}

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/user/routes",
        tag = "用户管理",
        security(
            ("bearer_auth" = [])
        ),
        responses(
            (status = 200, description = "获取用户有效路由成功", body = Vec<RouteInfo>),
        )
    )
)]
#[tracing::instrument(name = "获取用户的有效路由", skip(app_state, user))]
pub async fn get_user_effective_routes_handler(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<impl Responder, AppError> {
    let routes = get_user_effective_routes(&app_state.pool, &user.sub).await?;

    Ok(AppResponse::success_msg(routes, "获取用户有效路由成功"))
}
