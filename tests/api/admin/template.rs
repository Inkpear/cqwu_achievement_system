use crate::helper::{TestApp, TestUser, check_response_code_and_message};

#[tokio::test]
async fn create_template_success() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "name": "用户信息模板",
        "category": "用户管理",
        "description": "用于收集用户基本信息的模板",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "username": {
                        "type": "string",
                        "minLength": 3,
                        "maxLength": 20
                    },
                    "age": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 150
                    },
                    "email": {
                        "type": "string",
                        "format": "email"
                    }
                },
                "required": ["username", "email"]
            },
            "instance": {
                "username": "zhangsan",
                "age": 25,
                "email": "zhangsan@example.com"
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "收集模板创建成功");

    let data = &response["data"];
    assert!(data.get("template_id").is_some());
    assert_eq!(data["name"], "用户信息模板");
    assert_eq!(data["category"], "用户管理");
    assert_eq!(data["description"], "用于收集用户基本信息的模板");
    assert!(data.get("schema_def").is_some());
}

#[tokio::test]
async fn create_template_with_invalid_schema_is_rejected() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "name": "无效Schema模板",
        "category": "测试",
        "description": "带有无效schema的模板",
        "schema": {
            "schema_def": {
                "type": "invalid_type"  // 无效的 JSON Schema
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 400, "参数校验失败");
}

#[tokio::test]
async fn create_template_with_invalid_instance_is_rejected() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "name": "样例数据不匹配模板",
        "category": "测试",
        "description": "样例数据不符合Schema定义",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "username": {
                        "type": "string",
                        "minLength": 3
                    },
                    "age": {
                        "type": "integer"
                    }
                },
                "required": ["username", "age"]
            },
            "instance": {
                "username": "ab",  // 不满足 minLength: 3
                "age": "not_a_number"  // 类型错误
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 400, "参数校验失败");
}

#[tokio::test]
async fn create_template_without_instance_is_accepted() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "name": "无样例数据模板",
        "category": "测试",
        "description": "只有Schema定义，没有样例数据",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "field1": {
                        "type": "string"
                    }
                }
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "收集模板创建成功");
}

#[tokio::test]
async fn create_template_is_rejected_when_not_logged_in() {
    let app = TestApp::spawn().await;

    let body = serde_json::json!({
        "name": "测试模板",
        "category": "测试",
        "description": "测试描述",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 401, "未授权访问，请先登录");
}

#[tokio::test]
async fn create_template_is_rejected_when_name_is_invalid() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    // 测试空名称
    let body = serde_json::json!({
        "name": "",
        "category": "测试",
        "description": "测试描述",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 400, "参数校验失败");

    // 测试名称过长（超过100字符）
    let long_name = "a".repeat(101);
    let body = serde_json::json!({
        "name": long_name,
        "category": "测试",
        "description": "测试描述",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 400, "参数校验失败");
}

#[tokio::test]
async fn query_templates_returns_paginated_results() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    // 创建多个模板
    for i in 1..=5 {
        let body = serde_json::json!({
            "name": format!("模板{}", i),
            "category": "测试分类",
            "description": format!("描述{}", i),
            "schema": {
                "schema_def": {
                    "type": "object",
                    "properties": {
                        "field": {
                            "type": "string"
                        }
                    }
                }
            }
        });

        app.post_create_template(&body).await;
    }

    // 查询第一页（每页3条）
    let response = app
        .get_query_templates(None, None, None, 1, 3)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "success");

    let data = &response["data"];
    assert_eq!(data["items"].as_array().unwrap().len(), 3);
    assert!(data["total"].as_i64().unwrap() >= 5);
    assert_eq!(data["page"], 1);
    assert_eq!(data["page_size"], 3);
}

#[tokio::test]
async fn query_templates_by_name() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    // 创建一个特殊名称的模板
    let body = serde_json::json!({
        "name": "特殊查询模板XYZ",
        "category": "测试",
        "description": "用于测试名称查询",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    app.post_create_template(&body).await;

    // 按名称模糊查询
    let response = app
        .get_query_templates(None, Some("特殊查询"), None, 1, 10)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "success");

    let data = &response["data"];
    let list = data["items"].as_array().unwrap();
    assert!(list.len() >= 1);
    assert!(
        list.iter()
            .any(|item| item["name"].as_str().unwrap().contains("特殊查询"))
    );
}

#[tokio::test]
async fn query_templates_by_category() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    // 创建不同分类的模板
    let body1 = serde_json::json!({
        "name": "分类A模板",
        "category": "分类A",
        "description": "属于分类A",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    let body2 = serde_json::json!({
        "name": "分类B模板",
        "category": "分类B",
        "description": "属于分类B",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    app.post_create_template(&body1).await;
    app.post_create_template(&body2).await;

    // 按分类查询
    let response = app
        .get_query_templates(None, None, Some("分类A"), 1, 10)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "success");

    let data = &response["data"];
    let list = data["items"].as_array().unwrap();
    assert!(list.len() >= 1);
    for item in list {
        assert_eq!(item["category"], "分类A");
    }
}

#[tokio::test]
async fn query_templates_by_template_id() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    // 创建一个模板
    let body = serde_json::json!({
        "name": "ID查询测试模板",
        "category": "测试",
        "description": "用于测试ID查询",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    let create_response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    let template_id = create_response["data"]["template_id"]
        .as_str()
        .unwrap()
        .to_string();

    // 按ID查询
    let response = app
        .get_query_templates(Some(&template_id), None, None, 1, 10)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 200, "success");

    let data = &response["data"];
    let list = data["items"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["template_id"], template_id);
    assert_eq!(list[0]["name"], "ID查询测试模板");
}

#[tokio::test]
async fn admin_can_create_template() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;

    app.login(&user).await;

    let body = serde_json::json!({
        "name": "管理员创建的模板",
        "category": "测试",
        "description": "测试管理员权限",
        "schema": {
            "schema_def": {
                "type": "object"
            }
        }
    });

    let response = app
        .post_create_template(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "收集模板创建成功");
}
