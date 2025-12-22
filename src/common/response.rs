use actix_web::{HttpResponse, Responder, http::StatusCode};

#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[derive(serde::Serialize)]
#[cfg_attr(feature = "swagger", derive(ToSchema))]
pub struct AppResponse<T> {
    #[cfg_attr(feature = "swagger", schema(example = 200))]
    code: u16,

    #[cfg_attr(feature = "swagger", schema(example = "success"))]
    message: String,

    #[cfg_attr(feature = "swagger", schema(example = "null"))]
    data: Option<T>,
}

pub struct AppResponseBuilder<T> {
    response: AppResponse<T>,
}

impl<T> AppResponse<T>
where
    T: serde::Serialize,
{
    pub fn builder() -> AppResponseBuilder<T> {
        AppResponseBuilder::new()
    }

    pub fn success(data: T) -> Self {
        Self {
            code: StatusCode::OK.as_u16(),
            message: "success".into(),
            data: Some(data),
        }
    }

    pub fn success_msg(data: T, message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::OK.as_u16(),
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn created(data: T, msg: impl Into<String>) -> Self {
        Self {
            code: StatusCode::CREATED.as_u16(),
            message: msg.into(),
            data: Some(data),
        }
    }
}

impl AppResponse<()> {
    pub fn empty() -> AppResponseBuilder<()> {
        AppResponseBuilder::new()
    }

    pub fn ok() -> Self {
        Self {
            code: StatusCode::OK.as_u16(),
            message: "success".into(),
            data: None,
        }
    }

    pub fn ok_msg(message: &str) -> Self {
        Self {
            code: StatusCode::OK.as_u16(),
            data: None,
            message: message.into(),
        }
    }
}

impl<T> AppResponseBuilder<T>
where
    T: serde::Serialize,
{
    fn new() -> Self {
        let response = AppResponse {
            code: StatusCode::OK.as_u16(),
            message: "success".into(),
            data: None,
        };
        Self { response }
    }

    pub fn code(mut self, code: StatusCode) -> Self {
        self.response.code = code.as_u16();
        self
    }

    pub fn message(mut self, message: &str) -> Self {
        self.response.message = message.into();
        self
    }

    pub fn data(mut self, data: T) -> Self {
        self.response.data = Some(data);
        self
    }

    pub fn build(self) -> AppResponse<T> {
        self.response
    }
}

impl<T> Responder for AppResponse<T>
where
    T: serde::Serialize,
{
    type Body = actix_web::body::BoxBody;

    fn respond_to(self, _req: &actix_web::HttpRequest) -> actix_web::HttpResponse<Self::Body> {
        HttpResponse::build(
            StatusCode::from_u16(self.code).expect("a unknown status code was provided"),
        )
        .content_type("application/json")
        .json(self)
    }
}
