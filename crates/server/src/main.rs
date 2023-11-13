mod authentication;

use std::{
    fmt::Display,
    ops::Add,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
use rand::{thread_rng, Rng};
use rand_chacha::ChaCha8Rng;
use rand_core::{OsRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::types::PgInterval, PgPool};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio::time::interval;

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
        .route("/champions", get(get_champions))
        .route("/teams", get(get_teams))
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
    Extension(random): Extension<Random>,
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
    let now = OffsetDateTime::now_utc();
    let now_pdt = PrimitiveDateTime::new(now.date(), now.time());
    if now_pdt < row.can_get_points_time {
        return Err((
            StatusCode::FORBIDDEN,
            format!(
                "You're not ready to collect yet. next time is: {}",
                row.can_get_points_time
            ),
        ));
    }
    // TODO: that's duplicated, we need to remove from pool.
    match sqlx::query!(
        "UPDATE points_pool
        SET points = points - 1
        WHERE open_at < $1
        RETURNING points",
        now_pdt
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Err(e) => {
            return Err((
                StatusCode::FORBIDDEN,
                format!(
                    "Probably no points left in pool, or pool not open. ; error: {}",
                    e
                ),
            ))
        }
        Ok(row) => {
            if row.points <= 0 {
                //let mut random = random.lock().unwrap();
                let delay = 2;
                // random.gen_range(2..=6);
                match sqlx::query!(
                    "UPDATE points_pool
                    SET points = 200,
                    open_at = $1
                    ",
                    now_pdt + std::time::Duration::from_secs(delay * 60 * 60)
                )
                .execute(&mut *transaction)
                .await
                {
                    Err(e) => {
                        return Err((
                            StatusCode::FORBIDDEN,
                            format!(
                                "Probably no points left in pool, or pool is not open. ; error: {}",
                                e
                            ),
                        ))
                    }
                    Ok(row) => Ok(()),
                }?;
            }
        }
    };

    let row = match sqlx::query!(
        "UPDATE users
        SET points = points + 1,
            can_get_points_time = NOW() + CAST('12 seconds' AS INTERVAL)
        WHERE id = $1
        RETURNING points",
        dbg!(user.id)
    )
    .fetch_one(&mut *transaction)
    .await
    {
        Ok(todo) => Ok(todo),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }?;

    transaction
        .commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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

#[derive(Serialize)]
struct Champion {
    id: i64,
    team_id: i64,
    name: String,
}
async fn get_champions(Extension(database): Extension<PgPool>) -> impl IntoResponse {
    match sqlx::query_as!(Champion, "SELECT id, team_id, name from champions")
        .fetch_all(&database)
        .await
    {
        Ok(rows) => Ok((StatusCode::OK, axum::Json(rows))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}
#[derive(Serialize)]
struct Team {
    id: i64,
    name: String,
}
async fn get_teams(Extension(database): Extension<PgPool>) -> impl IntoResponse {
    match sqlx::query_as!(Team, "SELECT id, name from teams")
        .fetch_all(&database)
        .await
    {
        Ok(rows) => Ok((StatusCode::OK, axum::Json(rows))),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}
