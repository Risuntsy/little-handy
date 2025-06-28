use actix_files::NamedFile;
use actix_web::{
    get, post,
    web::{self},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    config::{get_config, get_service_config},
    git::{init_all_repo, GitClient, GitClientImpl},
    util::{verify_my_hash, verify_signature},
};

/// Serves the file if found and if the signature is valid.
#[get("/{service_name}/{file_path:.*}")]
async fn serve_file(
    req: HttpRequest,
    path: web::Path<(String, String)>,
    params: web::Query<ServeFileRequestParams>,
) -> impl Responder {
    let (service_name, file_path) = path.as_ref();

    info!(
        "Received request for service: {}, path: {}, with params: {:?}",
        service_name, file_path, &params
    );

    let service_config = get_service_config(service_name);

    // need verify signature
    if let Some(secret_key) = service_config.secret_key.as_ref() {
        if !verify_my_hash(
            &format!("{service_name}/{file_path}"),
            &params.sign,
            &secret_key,
        ) {
            warn!(
                "Invalid signature: service: {}, path: {}, sign: {}",
                service_name, file_path, &params.sign
            );
            return Ok(HttpResponse::Forbidden().body("Invalid signature"));
        }
    }

    let file_path = service_config.repo_path.join(file_path);

    if !file_path.exists() {
        warn!("File not found for path: {:?}", file_path);
        return Ok(HttpResponse::NotFound().body("File not found"));
    }

    info!("Serving file: {:?}", file_path);
    return NamedFile::open(file_path).map(|file| file.into_response(&req));
}

#[post("{service_name}/webhook")]
async fn webhook_handler(
    req: HttpRequest,
    payload: web::Bytes,
    path: web::Path<String>,
) -> impl Responder {
    let service_name = path.as_ref();

    let service_config = get_service_config(service_name);

    let signature = req.headers().get("X-Hub-Signature-256");
    if signature.is_none() {
        return HttpResponse::Forbidden().body("signature not found");
    }

    let signature = signature.unwrap().to_str();
    if signature.is_err() {
        return HttpResponse::Forbidden().body("Invalid signature");
    }

    let signature = signature.unwrap();

    let signature_verify_result = verify_signature(
        &payload,
        &signature[7..],
        &service_config.github_webhook_secret.as_bytes(),
        "sha256",
    );

    if signature_verify_result.is_err() || !signature_verify_result.unwrap() {
        return HttpResponse::InternalServerError().body("check signature failed");
    }

    GitClientImpl::new()
        .pull_repo(
            &service_config.repo_path,
            service_config
                .private_key_path
                .as_ref()
                .map(|p| p.as_path()),
        )
        .expect("Failed to pull repo");

    return HttpResponse::Ok().body("ok");
}

pub fn run_server() -> std::io::Result<()> {
    init_all_repo();
    actix_web::rt::System::new().block_on(async move {
        HttpServer::new(move || App::new().service(serve_file).service(webhook_handler))
            .workers(1)
            .bind(("0.0.0.0", get_config().server.port))?
            .run()
            .await
    })
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ServeFileRequestParams {
    pub sign: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GithubWebhookPayload {
    r#ref: String,
}
