use anyhow::Context;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use secrecy::{ExposeSecret, SecretString};

use crate::telemetry::spawn_blocking_with_tracing;

fn compute_password_hash(password: SecretString) -> Result<SecretString, anyhow::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(password.expose_secret().as_bytes(), &salt)?
        .to_string();

    Ok(SecretString::from(password_hash))
}

fn verify_password_hash(password: SecretString, hash: SecretString) -> Result<(), anyhow::Error> {
    let hash = PasswordHash::new(hash.expose_secret())?;

    Argon2::default()
        .verify_password(password.expose_secret().as_bytes(), &hash)
        .context("密码错误")?;

    Ok(())
}

pub async fn hash_password(password: SecretString) -> Result<SecretString, anyhow::Error> {
    spawn_blocking_with_tracing(move || compute_password_hash(password)).await?
}

#[tracing::instrument(name = "验证密码", skip(password, hash))]
pub async fn verify_password(
    password: SecretString,
    hash: SecretString,
) -> Result<(), anyhow::Error> {
    spawn_blocking_with_tracing(move || verify_password_hash(password, hash)).await?
}
