use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, DatabaseErrorCode},
        pagination::PageData,
    },
    domain::{HttpMethod, ROUTE_REGISTRY, RouteInfo},
    modules::admin::api_rule::models::{
        ApiRuleDTO, GrantUserApiRuleRequest, QueryUserApiRuleRequest,
    },
};

#[tracing::instrument(name = "创建用户 API 访问规则到数据库", skip(pool, req))]
pub async fn grant_user_api_access_rule(
    pool: &PgPool,
    req: &GrantUserApiRuleRequest,
    granted_by: &Uuid,
) -> Result<ApiRuleDTO, AppError> {
    check_api_rule_conflict(pool, req).await?;

    let row = sqlx::query!(
        r#"
        INSERT INTO sys_access_rule (user_id, api_pattern, http_method, expires_at, granted_by, description)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id, api_pattern, http_method)
        DO UPDATE SET expires_at = $4, granted_by = $5, description = $6
        RETURNING rule_id, created_at
        "#,
        req.user_id,
        req.api_pattern,
        req.http_method.as_str(),
        req.expires_at,
        granted_by,
        req.description,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if let Some(db_error) = e.as_database_error() {
            if Some(DatabaseErrorCode::FOREIGN_KEY_VIOLATION).eq(&db_error.code().as_deref()) {
                return AppError::DataNotFound("用户不存在".into());
            }
        }
        AppError::UnexpectedError(e.into())
    })?;

    let dto = ApiRuleDTO {
        rule_id: row.rule_id,
        api_pattern: req.api_pattern.clone(),
        http_method: req.http_method.clone(),
        expires_at: req.expires_at,
        created_at: row.created_at,
        granted_by: Some(*granted_by),
    };

    Ok(dto)
}

#[tracing::instrument(name = "检查 API 访问规则冲突", skip(pool, req))]
pub async fn check_api_rule_conflict(
    pool: &PgPool,
    req: &GrantUserApiRuleRequest,
) -> Result<(), AppError> {
    let row = sqlx::query!(
        r#"
        SELECT rule_id FROM sys_access_rule
        WHERE user_id = $1
            AND $2 LIKE (api_pattern || '%')
            AND (http_method = 'ALL' OR http_method = $3)
            AND (
                expires_at IS NULL 
                OR 
                (
                    $4::TIMESTAMPTZ IS NOT NULL 
                    AND expires_at >= $4::TIMESTAMPTZ
                )
            )
        "#,
        req.user_id,
        req.api_pattern,
        req.http_method.as_str(),
        req.expires_at,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if let Some(record) = row {
        tracing::warn!("已有更宽泛的规则: {:?}", record.rule_id);
        return Err(AppError::ApiRuleConflict(record.rule_id));
    }

    Ok(())
}

pub async fn check_api_rule_validity(
    pool: &PgPool,
    prefix: &str,
    method: &HttpMethod,
) -> Result<(), AppError> {
    let mut routes = get_registry_routes(pool).await?;
    do_filter_with_prefix(&mut routes, prefix, method);

    if routes.is_empty() {
        Err(AppError::DataNotFound("没有找到匹配的路由".into()))
    } else {
        Ok(())
    }
}

#[tracing::instrument(name = "从数据库撤销用户 API 访问规则", skip(pool))]
pub async fn revoke_user_api_access_rule(pool: &PgPool, rule_id: &Uuid) -> Result<(), AppError> {
    let row = sqlx::query!(
        r#"
        DELETE FROM sys_access_rule
        WHERE rule_id = $1
        "#,
        rule_id
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    if row.rows_affected() == 0 {
        tracing::warn!("未找到要撤销的规则: {}", rule_id);
        return Err(AppError::DataNotFound("API访问规则不存在".into()));
    }

    tracing::info!("API 访问规则已撤销: {}", rule_id);

    Ok(())
}

#[tracing::instrument(name = "从数据库查询用户 API 访问规则", skip(pool, req))]
pub async fn query_user_api_access_rules(
    pool: &PgPool,
    req: &QueryUserApiRuleRequest,
) -> Result<PageData<ApiRuleDTO>, AppError> {
    let count_result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM sys_access_rule
        WHERE user_id = $1
        "#,
        req.user_id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let total = count_result.count.unwrap_or(0);

    let rows = sqlx::query_as!(
        ApiRuleDTO,
        r#"
        SELECT 
            rule_id,
            api_pattern,
            http_method,
            expires_at,
            created_at,
            granted_by
        FROM sys_access_rule
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        req.user_id,
        req.page_size,
        req.offset()
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let page_data = PageData::from(rows, total, req.page, req.page_size);

    Ok(page_data)
}

#[tracing::instrument(name = "从数据库中获取路由注册表", skip(pool))]
pub async fn get_registry_routes(pool: &PgPool) -> Result<Vec<RouteInfo>, AppError> {
    let rows = sqlx::query!(
        r#"
            SELECT template_id, name, category
            FROM sys_template
            WHERE is_active = true
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;
    let mut routes = ROUTE_REGISTRY
        .read()
        .map_err(|e| AppError::UnexpectedError(anyhow::anyhow!("获取路由注册表锁失败: {}", e)))?
        .get_routes()
        .clone();

    for row in rows {
        let template_routes = build_template_route_info(row.template_id, row.name, row.category);
        routes.extend(template_routes);
    }
    let routes: Vec<RouteInfo> = routes.into_iter().collect();

    Ok(routes)
}

#[tracing::instrument(name = "获取用户有效的 API 访问规则", skip(pool, user_id))]
pub async fn get_effective_rules_for_user(
    pool: &PgPool,
    user_id: &Uuid,
) -> Result<Vec<(String, HttpMethod)>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT api_pattern, http_method
        FROM sys_access_rule
        WHERE user_id = $1
            AND (expires_at IS NULL OR expires_at >= NOW())
        "#,
        user_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::UnexpectedError(e.into()))?;

    let rules = rows
        .into_iter()
        .map(|row| (row.api_pattern, HttpMethod::from(row.http_method)))
        .collect();

    Ok(rules)
}

pub fn do_filter_with_prefix(routes: &mut Vec<RouteInfo>, prefix: &str, method: &HttpMethod) {
    routes.retain(|route| {
        route.path.starts_with(prefix) && (route.method == *method || *method == HttpMethod::ALL)
    });
}

pub async fn do_filter_with_user_exists_rules(
    pool: &PgPool,
    routes: &mut Vec<RouteInfo>,
    user_id: &Uuid,
) -> Result<(), AppError> {
    let user_rules = get_effective_rules_for_user(pool, user_id).await?;
    routes.retain(|route| {
        user_rules.iter().all(|(pattern, method)| {
            !route.path.starts_with(pattern)
                || (route.method != *method && *method != HttpMethod::ALL)
        })
    });

    Ok(())
}

fn build_template_route_info(template_id: Uuid, name: String, category: String) -> Vec<RouteInfo> {
    let route_name = format!("{}-{}", category, name);
    vec![
        RouteInfo {
            method: HttpMethod::POST,
            path: format!("/api/archive/{}/create/", template_id),
            category: category.clone(),
            description: format!("{}&用于创建{}", route_name, route_name),
        },
        RouteInfo {
            method: HttpMethod::GET,
            path: format!("/api/archive/{}/init_upload/", template_id),
            category: category.clone(),
            description: format!(
                "{}&如果{}需要文件， 则用于初始化该模板的上传会话",
                route_name, route_name
            ),
        },
        RouteInfo {
            method: HttpMethod::POST,
            path: format!("/api/archive/{}/presigned/", template_id),
            category: category.clone(),
            description: format!(
                "{}&如果{}需要文件， 则用于获取该模板的预签名上传URL",
                route_name, route_name
            ),
        },
        RouteInfo {
            method: HttpMethod::POST,
            path: format!("/api/archive/{}/query/", template_id),
            category: category.clone(),
            description: format!("{}&用于查询{}的归档记录", route_name, route_name),
        },
        RouteInfo {
            method: HttpMethod::DELETE,
            path: format!("/api/archive/{}/delete/", template_id),
            category: category.clone(),
            description: format!("{}&用于删除{}的归档记录", route_name, route_name),
        },
        RouteInfo {
            method: HttpMethod::GET,
            path: format!("/api/archive/{}/info/", template_id),
            category: category.clone(),
            description: format!("{}&用于获取{}的模板信息", route_name, route_name),
        },
    ]
}
