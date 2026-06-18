use crate::alarm::AlarmSystem;
use crate::clickhouse::ClickHouseClient;
use crate::models::{ApiResponse, SensorReading};
use crate::pointing::PointingAnalyzer;
use crate::transmission::TransmissionSimulator;
use crate::ws::{
    BroadcastAlarm, BroadcastPointingAccuracy, BroadcastSensorReading, BroadcastTransmissionError,
    WsBroadcastServer, WsSession,
};
use actix_web::{get, post, web, Error, HttpRequest, HttpResponse, Responder};
use actix_web_actors::ws;
use std::sync::Arc;

pub struct AppState {
    pub clickhouse: Arc<ClickHouseClient>,
    pub transmission_sim: Arc<TransmissionSimulator>,
    pub pointing_analyzer: Arc<PointingAnalyzer>,
    pub alarm_system: Arc<AlarmSystem>,
    pub ws_server: actix::Addr<WsBroadcastServer>,
}

#[get("/ws")]
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let ws = WsSession {
        id: 0,
        addr: state.ws_server.clone(),
    };
    let resp = ws::start(ws, &req, stream)?;
    Ok(resp)
}

#[get("/api/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "hunyi-astronomy-backend"
    }))
}

#[post("/api/sensor")]
pub async fn ingest_sensor_reading(
    state: web::Data<AppState>,
    reading: web::Json<SensorReading>,
) -> impl Responder {
    let mut reading = reading.into_inner();

    reading.cumulative_transmission_error = state
        .transmission_sim
        .compute_cumulative_error(&reading);

    if let Err(e) = state.clickhouse.insert_sensor_reading(&reading).await {
        log::error!("Failed to insert sensor reading: {}", e);
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!(
            "Failed to save reading: {}",
            e
        )));
    }

    let latitude = 34.25;
    let lst = 12.0 * 15.0;
    let pointing_result = state
        .pointing_analyzer
        .analyze_pointing_accuracy(&reading, latitude, lst);

    if let Err(e) = state.clickhouse.insert_pointing_accuracy(&pointing_result).await {
        log::error!("Failed to insert pointing accuracy: {}", e);
    }

    for axis_id in 1u8..=3 {
        if let Some(axis) = state.transmission_sim.get_axis_config(axis_id) {
            let angle = match axis_id {
                1 => reading.axis_azimuth_angle,
                2 => reading.axis_elevation_angle,
                3 => reading.axis_equatorial_angle,
                _ => 0.0,
            };
            let wear_levels: Vec<f64> = vec![
                reading.gear_wear_level_1,
                reading.gear_wear_level_2,
                reading.gear_wear_level_3,
            ];
            let mut te_result = state.transmission_sim.simulate_axis_transmission(
                axis,
                angle,
                1,
                &wear_levels,
                reading.temperature,
                5.0,
            );
            te_result.device_id = reading.device_id.clone();
            te_result.timestamp = reading.timestamp;
            if let Err(e) = state.clickhouse.insert_transmission_error(&te_result).await {
                log::error!("Failed to insert transmission error: {}", e);
            }
            state
                .ws_server
                .do_send(BroadcastTransmissionError { result: te_result });
        }
    }

    let alarms = state.alarm_system.process_reading(&reading);
    for alarm in &alarms {
        if let Err(e) = state.clickhouse.insert_alarm(alarm).await {
            log::error!("Failed to insert alarm: {}", e);
        }
        state.ws_server.do_send(BroadcastAlarm {
            alarm: alarm.clone(),
        });
    }

    state
        .ws_server
        .do_send(BroadcastSensorReading { reading: reading.clone() });
    state
        .ws_server
        .do_send(BroadcastPointingAccuracy {
            result: pointing_result.clone(),
        });

    HttpResponse::Ok().json(ApiResponse::ok(serde_json::json!({
        "reading": reading,
        "pointing": pointing_result,
        "alarms": alarms
    })))
}

#[get("/api/sensor/recent/{device_id}/{limit}")]
pub async fn get_recent_sensor_readings(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
) -> impl Responder {
    let (device_id, limit) = path.into_inner();
    match state
        .clickhouse
        .query_recent_sensor_readings(&device_id, limit)
        .await
    {
        Ok(readings) => HttpResponse::Ok().json(ApiResponse::ok(readings)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!(
            "Query failed: {}",
            e
        ))),
    }
}

#[get("/api/alarms/recent/{limit}")]
pub async fn get_recent_alarms(
    state: web::Data<AppState>,
    path: web::Path<u32>,
) -> impl Responder {
    let limit = path.into_inner();
    match state.clickhouse.query_recent_alarms(limit).await {
        Ok(alarms) => HttpResponse::Ok().json(ApiResponse::ok(alarms)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!(
            "Query failed: {}",
            e
        ))),
    }
}

#[get("/api/pointing/stats/{device_id}/{hours}")]
pub async fn get_pointing_stats(
    state: web::Data<AppState>,
    path: web::Path<(String, u32)>,
) -> impl Responder {
    let (device_id, hours) = path.into_inner();
    match state
        .clickhouse
        .query_pointing_stats_by_zone(&device_id, hours)
        .await
    {
        Ok(stats) => HttpResponse::Ok().json(ApiResponse::ok(stats)),
        Err(e) => HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!(
            "Query failed: {}",
            e
        ))),
    }
}

#[get("/api/transmission/simulate")]
pub async fn simulate_transmission(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let axis_id: u8 = query.get("axis").and_then(|s| s.parse().ok()).unwrap_or(1);
    let input_angle: f64 = query
        .get("angle")
        .and_then(|s| s.parse().ok())
        .unwrap_or(30.0);
    let wear: f64 = query
        .get("wear")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    let temperature: f64 = query
        .get("temp")
        .and_then(|s| s.parse().ok())
        .unwrap_or(25.0);

    if let Some(axis) = state.transmission_sim.get_axis_config(axis_id) {
        let wear_levels = vec![wear, wear, wear];
        let result = state
            .transmission_sim
            .simulate_axis_transmission(axis, input_angle, 1, &wear_levels, temperature, 5.0);
        HttpResponse::Ok().json(ApiResponse::ok(result))
    } else {
        HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid axis ID"))
    }
}
