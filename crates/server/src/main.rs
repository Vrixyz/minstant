mod login;

use std::fmt::Display;

use axum::{
    error_handling::HandleErrorLayer,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use axum_login::login_required;
use axum_login::{AuthManagerLayer, AuthUser};
use login::AuthSession;
use time::Duration;
use tower::{BoxError, ServiceBuilder};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

use crate::login::Backend;
use sqlx::PgPool;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[shuttle_runtime::main]
async fn axum(
    #[shuttle_shared_db::Postgres(local_uri = "{secrets.DATABASE_URL}")] pool: PgPool,
) -> shuttle_axum::ShuttleAxum {
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Migrations failed :(");

    let state = AppState { pool };

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(1)));

    // Auth service.
    //
    // This combines the session layer with our backend to establish the auth
    // service which will provide the auth session as a request extension.
    let backend = Backend::new(pool);
    let auth_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(AuthManagerLayer::new(backend, session_layer));

    let router: Router<AppState, axum::body::Body> = Router::new()
        .route("/points/collect", post(points_collect))
        .route("/points/assign/:id", post(points_assign))
        .route_layer(login_required!(Backend, login_url = "/login"))
        .route("/login", post(login::post_handlers::login))
        .route("/logout", get(login::get_handlers::logout))
        .with_state(state)
        .layer(auth_service.into());
    Ok(router.into())
}

async fn points_collect(session: AuthSession, State(state): State<AppState>) -> impl IntoResponse {
    let Some(session) = session.user else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "what".to_string()));
    };
    let user_id = session.id() as i32;
    let mut transaction = state
        .pool
        .begin()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let row = match sqlx::query!(
        r#"select can_get_points_time
        from users
        WHERE id = $1"#,
        user_id
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(todo) => Ok(todo),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }?;
    match sqlx::query!(
        "UPDATE users
        SET points = points + 1
        WHERE id = $1",
        user_id
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(todo) => {}
        Err(e) => return Err((StatusCode::BAD_REQUEST, e.to_string())),
    };
    let row = match sqlx::query!(
        "UPDATE users
        SET points = points + 1
        WHERE id = $1
        RETURNING points",
        user_id
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(row) => Ok(row),
        Err(e) => return Err((StatusCode::BAD_REQUEST, e.to_string())),
    }?;

    transaction
        .commit()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::OK, axum::Json(row.points)))
}

async fn points_assign(
    auth_session: AuthSession,
    State(state): State<AppState>,
    Path(champion_id): Path<i32>,
) -> impl IntoResponse {
    let Some(session) = auth_session.user else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "what".to_string()));
    };
    let user_id = session.id() as i32;
    let mut transaction = state
        .pool
        .begin()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match sqlx::query!(
        "UPDATE users
        SET points = points - 1
        WHERE id = $1",
        user_id
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(_) => {}
        Err(e) => return Err((StatusCode::BAD_REQUEST, e.to_string())),
    };
    let row = match sqlx::query!(
        "UPDATE champions
        SET points = points + 1
        WHERE id = $1
        RETURNING points",
        champion_id,
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(row) => Ok(row),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }?;
    transaction
        .commit()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok((StatusCode::OK, axum::Json(row.points)))
}
