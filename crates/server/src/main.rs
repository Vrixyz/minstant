mod authentication;

use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use authentication::AuthState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Extension, Router,
};
use rand_chacha::ChaCha8Rng;
use rand_core::{OsRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

type Random = Arc<Mutex<ChaCha8Rng>>;

#[shuttle_runtime::main]
async fn axum(
    #[shuttle_shared_db::Postgres(local_uri = "{secrets.DATABASE_URL}")] pool: PgPool,
) -> shuttle_axum::ShuttleAxum {
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Migrations failed :(");

    let random = ChaCha8Rng::seed_from_u64(OsRng.next_u64());
    let middleware_database = pool.clone();
    let router = Router::new()
        .route("/points/collect", post(points_collect))
        .route("/points/assign/:id", post(points_assign))
        .route("/users/signup", post(authentication::post_signup))
        .route("/users/login", post(authentication::post_login))
        .layer(middleware::from_fn(move |req, next| {
            authentication::auth(req, next, middleware_database.clone())
        }))
        .layer(Extension(pool))
        .layer(Extension(Arc::new(Mutex::new(random))));

    Ok(router.into())
}

async fn points_collect(
    Extension(mut current_user): Extension<AuthState>,
    Extension(database): Extension<PgPool>,
) -> impl IntoResponse {
    let Some(user) = current_user.get_user().await else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "You must be logged in.".to_string(),
        ));
    };
    let mut transaction = database
        .begin()
        .await
        .map_err(|e| (StatusCode::IM_A_TEAPOT, e.to_string()))?;
    let row = match sqlx::query!(
        r#"select can_get_points_time
        from users
        WHERE id = $1"#,
        user.id
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(todo) => Ok(todo),
        Err(e) => Err((StatusCode::LOCKED, e.to_string())),
    }?;
    dbg!(row);
    match sqlx::query!(
        "UPDATE users
        SET points = points + 1
        WHERE id = $1
        RETURNING points",
        dbg!(user.id)
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(todo) => {}
        Err(e) => return Err((StatusCode::EXPECTATION_FAILED, e.to_string())),
    };
    // TODO: that's duplicated, we need to remove from pool.
    let row = match sqlx::query!(
        "UPDATE users
        SET points = points + 1
        WHERE id = $1
        RETURNING points",
        user.id
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(row) => Ok(row),
        Err(e) => return Err((StatusCode::ALREADY_REPORTED, e.to_string())),
    }?;

    transaction
        .commit()
        .await
        .map_err(|e| (StatusCode::VARIANT_ALSO_NEGOTIATES, e.to_string()))?;
    Ok((StatusCode::OK, axum::Json(row.points)))
}

async fn points_assign(
    Extension(mut current_user): Extension<AuthState>,
    Extension(database): Extension<PgPool>,
    Path(champion_id): Path<i32>,
) -> impl IntoResponse {
    let Some(user) = current_user.get_user().await else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "You must be logged in.".to_string(),
        ));
    };
    let mut transaction = database
        .begin()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    match sqlx::query!(
        "UPDATE users
        SET points = points - 1
        WHERE id = $1",
        user.id
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
