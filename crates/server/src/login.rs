pub mod get_handlers;
pub mod post_handlers;

use std::{collections::HashMap, str::Utf8Error};

use async_trait::async_trait;
use axum::{error_handling::HandleErrorLayer, http::StatusCode, BoxError};
use axum_login::{AuthManagerLayer, AuthUser, AuthnBackend, UserId};
use password_auth::verify_password;
use serde::Deserialize;
use sqlx::{FromRow, PgPool};
use time::Duration;
use tower::ServiceBuilder;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

#[derive(Debug, Clone, FromRow)]
pub struct User {
    id: i64,
    pw_hash: Vec<u8>,
}

impl AuthUser for User {
    type Id = i64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        &self.pw_hash
    }
}

// Extractors.

// This allows us to extract the "next" field from the query string. We use this
// to redirect after log in.
#[derive(Debug, Deserialize)]
pub struct NextUrl {
    next: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Backend {
    pool: PgPool,
}

impl Backend {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = std::io::Error;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user: Option<Self::User> =
            sqlx::query!("select * from users where name = $1", creds.name)
                .fetch_one(&self.pool)
                .await
                .map(|row| {
                    Some(User {
                        id: row.id as i64,
                        pw_hash: row.pw_hash.as_bytes().to_vec(),
                    })
                })
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "wow"))?;
        let Some(user) = user else {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "wow"));
        };
        if verify_password(
            creds.password,
            std::str::from_utf8(&user.pw_hash)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, "wow"))?,
        )
        .is_err()
        {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "wow"));
        }
        return Ok(Some(user));
    }

    async fn get_user(&self, user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as("select * from users where id = $1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, "wow"))?;

        Ok(Some(user))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    name: String,
    password: String,
}
// We use a type alias for convenience. Note that we've supplied our concrete
// backend here.
pub type AuthSession = axum_login::AuthSession<Backend>;

pub fn layer(
    pool: PgPool,
) -> ServiceBuilder<
    tower::layer::util::Stack<AuthManagerLayer<Backend, MemoryStore>, tower::layer::util::Identity>,
> {
    // Session layer.
    //
    // This uses `tower-sessions` to establish a layer that will provide the session
    // as a request extension.
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)));

    // Auth service.
    //
    // This combines the session layer with our backend to establish the auth
    // service which will provide the auth session as a request extension.
    let backend = Backend::new(pool);
    let auth_service: ServiceBuilder<
        tower::layer::util::Stack<
            AuthManagerLayer<Backend, MemoryStore>,
            tower::layer::util::Identity,
        >,
    > = ServiceBuilder::new().layer(AuthManagerLayer::new(backend, session_layer));
    auth_service
}
