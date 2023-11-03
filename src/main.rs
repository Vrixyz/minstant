use std::fmt::Display;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
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
    let router = Router::new()
        .route("/points/collect", post(points_collect))
        .route("/points/assign/:id", post(points_assign))
        .with_state(state);

    Ok(router.into())
}

async fn points_collect(State(state): State<AppState>) -> impl IntoResponse {
    let user_id = 0;
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
    State(state): State<AppState>,
    Path(champion_id): Path<i32>,
) -> impl IntoResponse {
    let mut transaction = state
        .pool
        .begin()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    // TODO: retrieve player_id from authdata.
    let user_id = 0;
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
