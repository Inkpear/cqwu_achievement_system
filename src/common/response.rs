use actix_web::{HttpResponse, Responder, http::StatusCode};
use serde::ser::SerializeStruct;

pub struct AppResponse<T> {
    code: StatusCode,
    message: String,
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
            code: StatusCode::OK,
            message: "success".into(),
            data: Some(data),
        }
    }

    pub fn success_msg(data: T, message: String) -> Self {
        Self {
            code: StatusCode::OK,
            message,
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
            code: StatusCode::OK,
            message: "success".into(),
            data: None,
        }
    }
    
    pub fn ok_msg(message: &str) -> Self {
        Self {
            code: StatusCode::OK,
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
            code: StatusCode::OK,
            message: "success".into(),
            data: None,
        };
        Self { response }
    }

    pub fn code(mut self, code: StatusCode) -> Self {
        self.response.code = code;
        self
    }

    pub fn message(mut self, message: String) -> Self {
        self.response.message = message;
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

impl<T> serde::Serialize for AppResponse<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("AppResponse", 3)?;
        state.serialize_field("code", &self.code.as_u16())?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("data", &self.data)?;

        state.end()
    }
}

impl<T> Responder for AppResponse<T>
where
    T: serde::Serialize,
{
    type Body = actix_web::body::BoxBody;

    fn respond_to(self, _req: &actix_web::HttpRequest) -> actix_web::HttpResponse<Self::Body> {
        HttpResponse::build(self.code)
            .content_type("application/json")
            .json(self)
    }
}
