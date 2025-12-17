use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
    pub iat: usize,
    pub username: String,
}

#[derive(Deserialize, Clone)]
pub struct JwtConfig {
    secret: SecretString,
    expiration: usize,
}

impl JwtConfig {
    pub fn generate_jwt_token(
        &self,
        user_id: Uuid,
        username: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let issued_at = Utc::now();
        let expired_at = issued_at + Duration::seconds(self.expiration as i64);

        let claims = Claims {
            sub: user_id,
            exp: expired_at.timestamp() as usize,
            iat: issued_at.timestamp() as usize,
            username: username.to_string(),
        };

        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.expose_secret().as_bytes()),
        )?;

        Ok(token)
    }

    pub fn verify_jwt_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let decoded = jsonwebtoken::decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.expose_secret().as_bytes()),
            &Validation::default(),
        )?;

        Ok(decoded.claims)
    }
}
