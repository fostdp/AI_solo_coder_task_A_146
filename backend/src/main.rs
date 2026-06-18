mod alarm;
mod clickhouse;
mod handlers;
mod models;
mod pointing;
mod transmission;
mod ws;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use std::sync::Arc;

use crate::alarm::AlarmSystem;
use crate::clickhouse::ClickHouseClient;
use crate::handlers::{
    get_pointing_stats, get_recent_alarms, get_recent_sensor_readings, health_check,
    ingest_sensor_reading, simulate_transmission, ws_route, AppState,
};
use crate::pointing::PointingAnalyzer;
use crate::transmission::TransmissionSimulator;
use crate::ws::WsBroadcastServer;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    log::info!("Starting Hunyi Astronomy Backend...");

    let clickhouse_url =
        std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string());
    let clickhouse_db =
        std::env::var("CLICKHOUSE_DB").unwrap_or_else(|_| "hunyi_astronomy".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let ch_client = Arc::new(ClickHouseClient::new(&clickhouse_url, &clickhouse_db));
    let transmission_sim = Arc::new(TransmissionSimulator::new());
    let pointing_analyzer = Arc::new(PointingAnalyzer::new());
    let alarm_system = Arc::new(AlarmSystem::new());

    let ws_server = WsBroadcastServer::new().start();

    let app_state = web::Data::new(AppState {
        clickhouse: ch_client.clone(),
        transmission_sim: transmission_sim.clone(),
        pointing_analyzer: pointing_analyzer.clone(),
        alarm_system: alarm_system.clone(),
        ws_server: ws_server.clone(),
    });

    log::info!("Connecting to ClickHouse at {}", clickhouse_url);
    log::info!("Server listening on {}", bind_addr);

    HttpServer::new(move || {
        let cors = Cors::permissive();
        App::new()
            .wrap(cors)
            .app_data(app_state.clone())
            .service(health_check)
            .service(ws_route)
            .service(ingest_sensor_reading)
            .service(get_recent_sensor_readings)
            .service(get_recent_alarms)
            .service(get_pointing_stats)
            .service(simulate_transmission)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
