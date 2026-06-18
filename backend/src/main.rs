mod alarm_ws;
mod clickhouse;
mod dtu_receiver;
mod handlers;
mod models;
mod pointing_analyzer;
mod transmission_simulator;

use actix::Actor;
use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};
use alarm_ws::{AlarmEvaluator, WsBroadcastServer};
use clickhouse::ClickHouseClient;
use dtu_receiver::DtuReceiver;
use handlers::AppState;
use models::{AlarmConfig, GearParamsConfig, PipelineChannels};
use pointing_analyzer::PointingAnalyzer;
use std::sync::Arc;
use transmission_simulator::TransmissionSimulator;

const CHANNEL_CAPACITY: usize = 1024;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let config_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config");

    let gear_cfg = Arc::new(GearParamsConfig::load_from_file(
        &config_dir.join("gear_params.json"))?);
    let alarm_cfg = Arc::new(AlarmConfig::load_from_file(
        &config_dir.join("alarm_thresholds.json"))?);

    log::info!("配置加载完毕：gear_params={} 轴, alarm_thresholds OK", gear_cfg.axes.len());

    let ch_client = Arc::new(ClickHouseClient::new(
        std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://127.0.0.1:8123".to_string()),
        std::env::var("CLICKHOUSE_USER").unwrap_or_default(),
        std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
        std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "hunyi_analysis".to_string()),
    ));
    log::info!("ClickHouse 客户端已初始化");

    let (transmission_channels, rx_transmission, rx_pointing, rx_alarm) =
        PipelineChannels::new(CHANNEL_CAPACITY);

    let ws_server = WsBroadcastServer::new().start();

    let tx_alarm_ws = transmission_channels.to_alarm_ws.clone();
    let tx_ws_only = transmission_channels.to_alarm_ws.clone();

    let transmission = TransmissionSimulator::new(
        ch_client.clone(), gear_cfg.clone(),
        rx_transmission, tx_alarm_ws.clone(), tx_ws_only.clone()
    );
    tokio::spawn(async move { transmission.run().await });

    let alarm_evaluator = AlarmEvaluator::new(
        ch_client.clone(), alarm_cfg.clone(),
        rx_alarm, ws_server.clone()
    );
    tokio::spawn(async move { alarm_evaluator.run().await });

    let pointing = PointingAnalyzer::new(
        ch_client.clone(), gear_cfg.clone(),
        rx_pointing, tx_alarm_ws, tx_ws_only
    );
    tokio::spawn(async move { pointing.run().await });

    let dtu_receiver = Arc::new(DtuReceiver::new(
        ch_client.clone(), gear_cfg.clone(), transmission_channels,
    ));

    let app_state = web::Data::new(AppState {
        dtu_receiver, ch_client: ch_client.clone(), ws_server: ws_server.clone(),
    });

    let port: u16 = std::env::var("SERVER_PORT")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(8080);

    log::info!("浑仪分析引擎启动于 http://0.0.0.0:{}", port);
    log::info!("WebSocket 端点: ws://localhost:{}/ws", port);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
        App::new()
            .app_data(app_state.clone())
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .route("/health", web::get().to(handlers::health_check))
            .route("/api/v1/sensor/ingest", web::post().to(handlers::ingest_sensor_reading))
            .route("/api/v1/transmission/errors", web::get().to(handlers::query_transmission_errors))
            .route("/api/v1/pointing/accuracy", web::get().to(handlers::query_pointing_accuracy))
            .route("/api/v1/alarms", web::get().to(handlers::query_alarms))
            .route("/api/v1/gear/status", web::get().to(handlers::query_gear_status))
            .route("/ws", web::get().to(handlers::ws_handshake))
    })
    .workers(4)
    .bind(("0.0.0.0", port))?
    .run()
    .await?;

    Ok(())
}
