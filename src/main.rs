use axum::routing::{get, patch, post};
use axum::{middleware, Router};
use axum_prometheus::PrometheusMetricLayer;
use dotenvy::dotenv;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::auth::auth;
use crate::routes::{create_link, get_link_statistics, health, redirect, update_link};

mod auth;
mod routes;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "url_shortener=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Could not create database pool");

    let (prometheus_layer, metrics_handler) = PrometheusMetricLayer::pair();

    let app = Router::new()
        .route("/create", post(create_link))
        .route("/{id}/statistics", get(get_link_statistics))
        .route_layer(middleware::from_fn_with_state(db.clone(), auth))
        .route(
            "/{id}",
            patch(update_link)
                .route_layer(middleware::from_fn_with_state(db.clone(), auth))
                .get(redirect),
        )
        .route("/metrics", get(|| async move { metrics_handler.render() }))
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .layer(prometheus_layer)
        .with_state(db);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Could not initialize TcpListener");

    tracing::debug!(
        "listening on {}",
        listener.local_addr().expect("Could not get local address")
    );

    axum::serve(listener, app.into_make_service())
        .await
        .expect("could not start server");

    Ok(())
}
