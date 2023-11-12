use std::{error::Error, fmt::Display, str::FromStr};

use crate::Random;
use axum::{
    body, extract,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use password_auth::{generate_hash, verify_password};
use rand_core::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::{Database, PgPool};

#[derive(Debug)]
pub(crate) enum MultipartError {
    NoName,
    InvalidValue,
    ReadError,
}

impl Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultipartError::NoName => f.write_str("No named field in multipart"),
            MultipartError::InvalidValue => f.write_str("Invalid value in multipart"),
            MultipartError::ReadError => f.write_str("Reading multipart error"),
        }
    }
}

impl Error for MultipartError {}

#[derive(Debug, Serialize)]
pub(crate) enum SignupError {
    NameExists,
    InvalidName,
    PasswordsDoNotMatch,
    MissingDetails,
    InvalidPassword,
    InternalError,
}

impl Display for SignupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignupError::InvalidName => f.write_str("Invalid user name"),
            SignupError::NameExists => f.write_str("User name already exists"),
            SignupError::PasswordsDoNotMatch => f.write_str("Passwords do not match"),
            SignupError::MissingDetails => f.write_str("Missing Details"),
            SignupError::InvalidPassword => f.write_str("Invalid Password"),
            SignupError::InternalError => f.write_str("Internal Error"),
        }
    }
}

impl Error for SignupError {}

impl IntoResponse for SignupError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}

#[derive(Debug)]
pub(crate) enum LoginError {
    MissingDetails,
    UserDoesNotExist,
    WrongPassword,
}

impl Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::UserDoesNotExist => f.write_str("User does not exist"),
            LoginError::MissingDetails => f.write_str("Missing details"),
            LoginError::WrongPassword => f.write_str("Wrong password"),
        }
    }
}
impl IntoResponse for LoginError {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}

impl Error for LoginError {}

#[derive(Serialize, Deserialize)]
pub struct SignupData {
    pub name: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginData {
    pub name: String,
    pub password: String,
}

pub async fn post_signup(
    Extension(random): Extension<Random>,
    Extension(database): Extension<PgPool>,
    Json::<SignupData>(SignupData { name, password }): extract::Json<SignupData>,
) -> impl IntoResponse {
    fn valid_username(name: &str) -> bool {
        (1..20).contains(&name.len())
            && name
                .chars()
                .all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-'))
    }

    if !valid_username(&name) {
        return Err(SignupError::InvalidName);
    }
    const INSERT_QUERY: &str = "INSERT INTO users (name, password) VALUES ($1, $2) RETURNING id;";

    // Hash password to PHC string ($pbkdf2-sha256$...)
    let hashed_password = generate_hash(password);
    let fetch_one = sqlx::query_as(INSERT_QUERY)
        .bind(name)
        .bind(hashed_password)
        .fetch_one(&database)
        .await;
    let user_id: i32 = match fetch_one {
        Ok((user_id,)) => user_id,
        Err(sqlx::Error::Database(database)) if database.constraint() == Some("users_name_key") => {
            return Err(SignupError::NameExists);
        }
        Err(_err) => {
            return Err(SignupError::InternalError);
        }
    };
    let session = new_session(&database, random, user_id).await;
    let response = session.to_response();
    Ok(response)
}

pub(crate) async fn post_login(
    Extension(random): Extension<Random>,
    Extension(database): Extension<PgPool>,
    Json::<LoginData>(LoginData { name, password }): extract::Json<LoginData>,
) -> impl IntoResponse {
    const LOGIN_QUERY: &str = "SELECT id, password FROM users WHERE users.name = $1";

    let row: Option<(i32, String)> = sqlx::query_as(LOGIN_QUERY)
        .bind(name)
        .fetch_optional(&database)
        .await
        .unwrap();

    let (user_id, correct_hashed_password) = if let Some(row) = row {
        row
    } else {
        return Err(LoginError::UserDoesNotExist);
    };

    // Verify password against PHC string
    if let Err(_err) = verify_password(password, &correct_hashed_password) {
        return Err(LoginError::WrongPassword);
    }
    let session = new_session(&database, random, user_id).await;
    let response = session.to_response();

    Ok(response)
}

const USER_COOKIE_NAME: &str = "user_token";
const COOKIE_MAX_AGE: &str = "9999999";

#[derive(Clone, Copy)]
pub(crate) struct SessionToken(u128);

impl FromStr for SessionToken {
    type Err = <u128 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl SessionToken {
    pub fn generate_new(random: Random) -> Self {
        let mut u128_pool = [0u8; 16];
        random.lock().unwrap().fill_bytes(&mut u128_pool);
        Self(u128::from_le_bytes(u128_pool))
    }

    pub fn into_cookie_value(self) -> String {
        // TODO Opportunity for a smaller format that is still a valid cookie value
        self.0.to_string()
    }

    pub fn into_database_value(self) -> Vec<u8> {
        self.0.to_le_bytes().to_vec()
    }
    pub fn to_response(self) -> http::Response<String> {
        http::Response::builder()
            .status(http::StatusCode::OK)
            .header("Location", "/")
            .header(
                "Set-Cookie",
                format!(
                    "{}={}; Max-Age={}; Path=/",
                    USER_COOKIE_NAME,
                    self.into_cookie_value(),
                    COOKIE_MAX_AGE
                ),
            )
            .body("".to_string())
            .unwrap()
    }
}

#[derive(Clone)]
pub(crate) struct User {
    pub id: i32,
    pub name: String,
}

#[derive(Clone)]
pub(crate) struct AuthState(Option<(SessionToken, Option<User>, PgPool)>);

impl AuthState {
    pub async fn get_user(&mut self) -> Option<&User> {
        let (session_token, store, database) = self.0.as_mut()?;
        if store.is_none() {
            const QUERY: &str =
                "SELECT id, name FROM users JOIN sessions ON user_id = id WHERE session_token = $1;";

            let user: Option<(i32, String)> = sqlx::query_as(QUERY)
                .bind(&session_token.into_database_value())
                .fetch_optional(&*database)
                .await
                .unwrap();

            if let Some((id, name)) = user {
                *store = Some(User { id, name });
            }
        }
        store.as_ref()
    }
}

/// TODO date
pub(crate) async fn new_session(database: &PgPool, random: Random, user_id: i32) -> SessionToken {
    const QUERY: &str = "INSERT INTO sessions (session_token, user_id) VALUES ($1, $2);";

    let session_token = SessionToken::generate_new(random);

    let _result = sqlx::query(QUERY)
        .bind(&session_token.into_database_value())
        .bind(user_id)
        .execute(database)
        .await
        .unwrap();

    session_token
}

/// **AUTH MIDDLEWARE**
pub(crate) async fn auth<B>(
    mut req: http::Request<B>,
    next: axum::middleware::Next<B>,
    database: PgPool,
) -> axum::response::Response {
    let session_token = req
        .headers()
        .get_all("Cookie")
        .iter()
        .filter_map(|cookie| {
            cookie
                .to_str()
                .ok()
                .and_then(|cookie| cookie.parse::<cookie::Cookie>().ok())
        })
        .find_map(|cookie| {
            (cookie.name() == USER_COOKIE_NAME).then(move || cookie.value().to_owned())
        })
        .and_then(|cookie_value| cookie_value.parse::<SessionToken>().ok());

    req.extensions_mut()
        .insert(AuthState(session_token.map(|v| (v, None, database))));

    next.run(req).await
}
