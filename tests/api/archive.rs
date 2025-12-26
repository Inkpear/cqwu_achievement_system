use uuid::Uuid;

use crate::helper::{
    TestApp, TestUser, check_response_code_and_message, generate_a_dummy_file_content,
};

#[tokio::test]
async fn create_archive_and_query_success() {
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

    let template_id = response["data"]["template_id"]
        .as_str()
        .expect("template_id should be a string");

    for i in 0..10 {
        let record_body = serde_json::json!({
            "data": {
                "username": format!("user{}", i),
                "age": 20 + i,
                "email": format!("user{}@example.com", i)
            }
        });
        let archive_response = app
            .post_create_archive_record(template_id, &record_body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse JSON response");

        check_response_code_and_message(&archive_response, 201, "创建归档记录成功");
    }

    let filters = vec![("username", "LIKE", "u%"), ("age", "GT", "26")];
    let body = serde_json::json!({
        "filters": filters.iter().map(|(field, op, value)| {
            serde_json::json!({
                "field": field,
                "operator": op,
                "value": value
            })
        }).collect::<Vec<serde_json::Value>>()
    });

    let query_response = app.post_query_archive_records(template_id, &body).await;
    assert_eq!(query_response.status().as_u16(), 200);

    let query_response = query_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&query_response, 200, "查询归档记录成功");

    let items = query_response["data"]["items"]
        .as_array()
        .expect("items should be an array");

    assert_eq!(items.len(), 3);
}

#[tokio::test]
async fn query_archive_is_safe_from_sql_injection() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let template_body = serde_json::json!({
        "name": "安全测试模板", "category": "测试", "description": "...",
        "schema": { "schema_def": { "type": "object" }, "instance": {} }
    });
    let t_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let tid = t_res["data"]["template_id"].as_str().unwrap();

    app.post_create_archive_record(tid, &serde_json::json!({ "data": { "name": "Alice" } }))
        .await;

    let injection_value = "Alice'; DROP TABLE sys_user; --";

    let query_body = serde_json::json!({
        "filters": [
            { "field": "name", "operator": "LIKE", "value": injection_value }
        ]
    });

    let res = app.post_query_archive_records(tid, &query_body).await;
    assert_eq!(res.status().as_u16(), 200);

    let json: serde_json::Value = res.json().await.unwrap();
    let items = json["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 0);

    let user_check = sqlx::query!("SELECT count(*) as count FROM sys_user")
        .fetch_one(&app.db_pool)
        .await;
    assert!(user_check.is_ok());
}

#[tokio::test]
async fn create_archive_fails_when_data_violates_schema() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let template_body = serde_json::json!({
        "name": "严格校验模板",
        "category": "测试",
        "description": "...",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "age": { "type": "integer", "minimum": 0 },
                    "email": { "type": "string" }
                },
                "required": ["email"]
            },
            "instance": { "age": 18, "email": "test@test.com" }
        }
    });
    let template_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let template_id = template_res["data"]["template_id"].as_str().unwrap();

    // 缺少必填字段
    let invalid_body_1 = serde_json::json!({ "data": { "age": 18 } });
    let res1 = app
        .post_create_archive_record(template_id, &invalid_body_1)
        .await
        .json()
        .await
        .unwrap();
    check_response_code_and_message(&res1, 400, "参数校验失败");

    // 类型/约束错误
    let invalid_body_2 = serde_json::json!({ "data": { "age": -5, "email": "a@b.com" } });
    let res2 = app
        .post_create_archive_record(template_id, &invalid_body_2)
        .await
        .json()
        .await
        .unwrap();
    check_response_code_and_message(&res2, 400, "参数校验失败");
}

#[tokio::test]
async fn grant_user_template_access_and_user_can_create_and_query_archive() {
    let mut app = TestApp::spawn().await;
    let admin_user = TestUser::default_admin(&app.db_pool).await;
    let mut normal_user = TestUser::new();
    normal_user.store(&app.db_pool).await;

    app.login(&admin_user).await;

    let template_body = serde_json::json!({
        "name": "用户访问测试模板",
        "category": "测试",
        "description": "...",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "info": { "type": "string" }
                },
                "required": ["info"]
            },
            "instance": { "info": "test" }
        }
    });
    let template_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let template_id = template_res["data"]["template_id"].as_str().unwrap();

    let body = serde_json::json!({
        "user_id": normal_user.user_id.unwrap().to_string(),
        "api_pattern": format!("/api/archive/{}/", template_id),
        "http_method": "ALL",
        "description": "允许访问归档模板接口",
        "expires_at": null
    });

    let grant_res = app
        .post_grant_user_api_rule(&body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    check_response_code_and_message(&grant_res, 201, "授予用户 API 访问规则成功");

    app.login(&normal_user).await;

    let record_body = serde_json::json!({
        "data": { "info": "normal user data" }
    });
    let create_res = app
        .post_create_archive_record(template_id, &record_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    check_response_code_and_message(&create_res, 201, "创建归档记录成功");

    let query_res = app
        .post_query_archive_records(template_id, &serde_json::json!({}))
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    check_response_code_and_message(&query_res, 200, "查询归档记录成功");

    let items = query_res["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test]
async fn a_normal_user_can_not_create_or_query_archive_without_permission() {
    let mut app = TestApp::spawn().await;
    let mut normal_user = TestUser::new();
    normal_user.store(&app.db_pool).await;

    app.login(&normal_user).await;

    let template_id = uuid::Uuid::new_v4();

    let record_body = serde_json::json!({
        "data": { "info": "should not be created" }
    });
    let create_res = app
        .post_create_archive_record(&template_id.to_string(), &record_body)
        .await;
    assert_eq!(create_res.status().as_u16(), 403);

    let query_res = app
        .post_query_archive_records(&template_id.to_string(), &serde_json::json!({}))
        .await;
    assert_eq!(query_res.status().as_u16(), 403);
}

#[tokio::test]
async fn submit_a_invalid_archive_query_missing_400() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let template_body = serde_json::json!({
        "name": "无效查询测试模板",
        "category": "测试",
        "description": "...",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "field1": { "type": "string" }
                },
                "required": ["field1"]
            },
            "instance": { "field1": "value1" }
        }
    });
    let template_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let template_id = template_res["data"]["template_id"].as_str().unwrap();

    let invalid_query_body = serde_json::json!({
        "filters": [
            { "field": "field1", "operator": "GT", "value": "value" }
        ]
    });

    let query_res = app
        .post_query_archive_records(template_id, &invalid_query_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();

    check_response_code_and_message(&query_res, 400, "构造JSON Schema 查询失败");
}

#[tokio::test]
async fn create_a_file_template_and_query_it_success() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let body = serde_json::json!({
        "name": "文件模板",
        "category": "文件收集",
        "description": "用于收集文件的模板",
        "schema": {
            "schema_def": {
                "name": "文件信息定义",
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }
        },
        "schema_files": [
            {
                "field": "head",
                "title": "用户头像",
                "file_config": {
                    "allowed_types": [".jpg", ".pdf"],
                    "quota": 2,
                    "max_size": 1048576,
                    "required": true,
                }
            }
        ]
    });

    let response = app.post_create_template(&body).await;
    assert_eq!(response.status().as_u16(), 201);

    let response = response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&response, 201, "收集模板创建成功");

    let template_id = response["data"]["template_id"]
        .as_str()
        .expect("template_id should be a string");
    let query_response = app
        .get_query_templates(Some(template_id), None, None, 1, 10)
        .await;
    assert_eq!(query_response.status().as_u16(), 200);
    let query_response = query_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    check_response_code_and_message(&query_response, 200, "查询收集模板成功");
    let prop = &query_response["data"]["items"][0]["schema_def"]["properties"];
    assert_eq!(prop["head"]["type"], "array");
    assert_eq!(prop["head"]["items"]["type"], "string");
    assert_eq!(prop["head"]["items"]["format"], "file-id");
    assert_eq!(prop["head"]["maxItems"], 2);
    assert_eq!(prop["head"]["title"], "用户头像");
    // dbg!(query_response);
}

#[tokio::test]
async fn init_upload_session_and_get_upload_urls_success_and_get_again_returns_403() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let body = serde_json::json!({
        "name": "文件上传模板",
        "category": "文件收集",
        "description": "用于收集文件的模板",
        "schema": {
            "schema_def": {
                "name": "文件信息定义",
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }
        },
        "schema_files": [
            {
                "field": "document",
                "title": "文档文件",
                "file_config": {
                    "allowed_types": [".docx", ".pdf", ".png"],
                    "quota": 1,
                    "max_size": 2097152,
                    "required": true,
                }
            }
        ]
    });

    let response = app.post_create_template(&body).await;
    assert_eq!(response.status().as_u16(), 201);

    let response = response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    let template_id = response["data"]["template_id"]
        .as_str()
        .expect("template_id should be a string");

    let init_response = app.get_init_upload_session(template_id).await;
    assert_eq!(init_response.status().as_u16(), 201);

    let init_response = init_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&init_response, 201, "初始化上传会话成功");

    let upload_session_id = init_response["data"]
        .as_str()
        .expect("upload_session_id should be a string");
    assert!(!upload_session_id.is_empty());
    let upload_session_id = Uuid::parse_str(upload_session_id).unwrap();
    let presigned_body = serde_json::json!({
        "session_id": upload_session_id,
        "field": "document",
        "filename": "testfile.png",
        "content_length": 270202
    });
    let presigned_response = app
        .post_presigned_upload_url(template_id, &presigned_body)
        .await;
    assert_eq!(presigned_response.status().as_u16(), 201);
    let presigned_response = presigned_response
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");

    check_response_code_and_message(&presigned_response, 201, "获取预签名上传URL成功");

    let presigned_body_2 = serde_json::json!({
        "session_id": upload_session_id,
        "field": "document",
        "filename": "testfile2.png",
        "content_length": 270202
    });
    let presigned_response_2 = app
        .post_presigned_upload_url(template_id, &presigned_body_2)
        .await;
    assert_eq!(presigned_response_2.status().as_u16(), 403);
    let presigned_response_2 = presigned_response_2
        .json::<serde_json::Value>()
        .await
        .expect("Failed to parse JSON response");
    check_response_code_and_message(&presigned_response_2, 403, "该字段配额已用完");
}

async fn create_a_file_record_return_record_id(app: &mut TestApp) -> Uuid {
    let template_body = serde_json::json!({
        "name": "文件归档模板",
        "category": "文件收集",
        "description": "用于收集文件的模板",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" }
                },
                "required": ["title"]
            }
        },
        "schema_files": [
            {
                "field": "attachment",
                "title": "附件文件",
                "file_config": {
                    "allowed_types": [".pdf", ".txt"],
                    "quota": 2,
                    "max_size": 1048576,
                    "required": true,
                }
            }
        ]
    });
    let template_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    check_response_code_and_message(&template_res, 201, "收集模板创建成功");

    let template_id = template_res["data"]["template_id"].as_str().unwrap();
    let template_id_uuid = Uuid::parse_str(template_id).unwrap();

    let exists = sqlx::query!(
        "SELECT 1 as exists FROM sys_template WHERE template_id = $1",
        template_id_uuid
    )
    .fetch_optional(&app.db_pool)
    .await
    .unwrap();

    assert!(exists.is_some(), "模板在提交前就应该存在于数据库中！");

    let init_res = app.get_init_upload_session(template_id).await;
    let init_res = init_res.json::<serde_json::Value>().await.unwrap();

    check_response_code_and_message(&init_res, 201, "初始化上传会话成功");

    let session_id = init_res["data"].as_str().unwrap();
    let session_id = Uuid::parse_str(session_id).unwrap();
    let dummy_file_content = generate_a_dummy_file_content(512 * 1024);
    let filename = "report.pdf";
    let content_type = mime_guess::from_path(filename)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    let presigned_body = serde_json::json!({
        "session_id": session_id,
        "field": "attachment",
        "filename": filename,
        "content_length": dummy_file_content.len() as i64,
    });

    let presigned_res = app
        .post_presigned_upload_url(template_id, &presigned_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();

    let upload_url = presigned_res["data"]["url"].as_str().unwrap();
    let file_id = presigned_res["data"]["file_id"].as_str().unwrap();
    let upload_res = app
        .put_to_upload_file(upload_url, &dummy_file_content, &content_type, filename)
        .await;

    assert_eq!(upload_res.status().as_u16(), 200);
    let record_body = serde_json::json!({
        "data": {
            "title": "文件归档示例",
            "attachment": [file_id]
        },
        "session_id": session_id
    });
    let record_res = app
        .post_create_archive_record(template_id, &record_body)
        .await
        .json()
        .await
        .unwrap();
    check_response_code_and_message(&record_res, 201, "创建归档记录成功");

    let file_url = record_res["data"]["data"]["attachment"][0]
        .as_str()
        .unwrap();

    let view_url_res = reqwest::Client::new()
        .get(file_url)
        .send()
        .await
        .expect("Failed to send GET request to view file");
    assert_eq!(view_url_res.status().as_u16(), 200);

    let file_name = view_url_res
        .headers()
        .get("content-disposition")
        .expect("Missing Content-Disposition header")
        .to_str()
        .expect("Failed to convert header value to str");
    assert!(file_name.contains("report.pdf"));

    let content = view_url_res
        .bytes()
        .await
        .expect("Failed to read file content");
    assert_eq!(content, dummy_file_content);
    
    record_res["data"]["record_id"]
        .as_str()
        .and_then(|id_str| Uuid::parse_str(id_str).ok())
        .expect("record_id should be a valid UUID")
}

#[tokio::test]
async fn create_a_file_record_success() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    create_a_file_record_return_record_id(&mut app).await;
}

#[tokio::test]
async fn create_a_file_record_fail_with_a_random_file_id() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let template_body = serde_json::json!({
        "name": "文件归档模板",
        "category": "文件收集",
        "description": "用于收集文件的模板",
        "schema": {
            "schema_def": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" }
                },
                "required": ["title"]
            }
        },
        "schema_files": [
            {
                "field": "attachment",
                "title": "附件文件",
                "file_config": {
                    "allowed_types": [".pdf", ".txt"],
                    "quota": 2,
                    "max_size": 1048576,
                    "required": true,
                }
            }
        ]
    });
    let template_res = app
        .post_create_template(&template_body)
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    check_response_code_and_message(&template_res, 201, "收集模板创建成功");

    let template_id = template_res["data"]["template_id"].as_str().unwrap();

    let record_body = serde_json::json!({
        "data": {
            "title": "文件归档示例",
            "attachment": [Uuid::new_v4().to_string()]
        }
    });
    let record_res = app
        .post_create_archive_record(template_id, &record_body)
        .await
        .json()
        .await
        .unwrap();
    check_response_code_and_message(&record_res, 400, "请上传必要的文件");
}

#[tokio::test]
async fn create_a_file_record_and_delete_it_success() {
    let mut app = TestApp::spawn().await;
    let user = TestUser::default_admin(&app.db_pool).await;
    app.login(&user).await;

    let record_id = create_a_file_record_return_record_id(&mut app).await;

    let delete_res = app
        .delete_archive_record(&record_id.to_string())
        .await
        .json::<serde_json::Value>()
        .await
        .unwrap();
    check_response_code_and_message(&delete_res, 200, "删除归档记录成功");
}