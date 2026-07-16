//! job-runner service entrypoint.
//!
//! Routes are skeletal; substance lands with the provisioning vertical
//! slice (docs/13-ownership-review.md). /health and /metrics come from
//! service-kit so the k8s manifests are truthful.

use axum::Router;

#[tokio::main]
async fn main() -> Result<(), service_kit::ServiceError> {
    let app = Router::new();
    service_kit::run("job-runner", 8082, app).await
}
