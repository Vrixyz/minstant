use super::*;
use axum::{http::StatusCode, response::IntoResponse, Form};

// This is our POST log in handler.
//
// It uses our auth session and the form URL encoded credentials to authenticate
// and log and user in.
//
// We've also implemented a basic scheme for displaying errors and redirecting
// on success.
pub async fn login(
    mut auth_session: AuthSession,
    Form(creds): Form<Credentials>,
) -> impl IntoResponse {
    let user = match auth_session.authenticate(creds.clone()).await {
        Ok(Some(user)) => user,
        Ok(None) => return "Invalid credentials".into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if auth_session.login(&user).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    StatusCode::OK.into_response()
}
