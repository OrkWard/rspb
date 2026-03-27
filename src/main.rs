use config::Config;
use log::info;
use mimalloc::MiMalloc;
use model::DataTrees;
use tokio::signal::unix::{signal, SignalKind};

use warp::Filter;
mod base32;
mod config;
mod controller;
mod highlighter;
mod markdown;
mod model;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
    let config: Config = config::Config::load(None).await.unwrap_or_default();
    let help = markdown::render(
        tokio::fs::read_to_string("README.md")
            .await
            .unwrap_or_else(|_| String::from("cmd | curl -F c=@- http://pb.lan/")),
    );
    highlighter::highlight_lines(&String::from(""), &String::from("rs"));
    flexi_logger::Logger::try_with_env_or_str("info")
        .unwrap()
        .format(flexi_logger::colored_default_format)
        .start()
        .unwrap();
    let sled_config = sled::Config::default()
        .cache_capacity(config.db_cache_capacity)
        .use_compression(true)
        .path("db");
    let db: sled::Db = sled_config.open().unwrap();
    let model: model::DataTrees = DataTrees::new(db);
    let model_filter = warp::any().map(move || model.clone());
    let help_route = warp::path::end()
        .and(warp::get())
        .map(move || warp::reply::html(help.clone()));
    let upload_route = warp::path::end()
        .or(warp::path("u"))
        .unify()
        .and(warp::path::full())
        .and(warp::post())
        .and(warp::multipart::form().max_length(config.max_length))
        .and(model_filter.clone())
        .and(warp::header::<String>("host"))
        .and_then(controller::upload);
    let custom_url_route = warp::post()
        .and(warp::path!(String))
        .and(warp::path::full())
        .and(warp::multipart::form().max_length(config.max_length))
        .and(model_filter.clone())
        .and(warp::header::<String>("host"))
        .and_then(controller::custom_url_upload);
    let view_route = warp::get()
        .and(warp::path!(String))
        .and(model_filter.clone())
        .and_then(controller::view_data);
    let delete_route = warp::delete()
        .and(warp::path!(String))
        .and(model_filter.clone())
        .and_then(controller::delete_data);
    let update_route = warp::put()
        .and(warp::path!(String))
        .and(model_filter.clone())
        .and(warp::header::<String>("host"))
        .and(warp::multipart::form().max_length(config.max_length))
        .and_then(controller::update_data);

    let route = upload_route
        .or(view_route)
        .or(delete_route)
        .or(custom_url_route)
        .or(update_route)
        .or(help_route)
        .with(warp::log("rspb"));

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    let (addr, server) = warp::serve(route)
        .bind_with_graceful_shutdown((config.ip, config.port), async move {
            shutdown_rx.await.ok();
        });

    info!("Server starting on {}", addr);

    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");

    tokio::select! {
        _ = server => {
            info!("Server stopped gracefully");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received SIGINT (Ctrl+C), starting graceful shutdown...");
            let _ = shutdown_tx.send(());
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM, starting graceful shutdown...");
            let _ = shutdown_tx.send(());
        }
    }
}
