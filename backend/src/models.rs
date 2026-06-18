use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub axis_azimuth_angle: f64,
    pub axis_elevation_angle: f64,
    pub axis_equatorial_angle: f64,
    pub gear_meshing_error_1: f64,
    pub gear_meshing_error_2: f64,
    pub gear_meshing_error_3: f64,
    pub bearing_clearance_1: f64,
    pub bearing_clearance_2: f64,
    pub bearing_clearance_3: f64,
    pub observed_star_ra: f64,
    pub observed_star_dec: f64,
    pub theoretical_ra: f64,
    pub theoretical_dec: f64,
    pub ra_deviation: f64,
    pub dec_deviation: f64,
    pub cumulative_transmission_error: f64,
    pub gear_wear_level_1: f64,
    pub gear_wear_level_2: f64,
    pub gear_wear_level_3: f64,
    pub temperature: f64,
    pub humidity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmissionErrorResult {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub axis_id: u8,
    pub input_angle: f64,
    pub output_angle: f64,
    pub theoretical_ratio: f64,
    pub actual_ratio: f64,
    pub single_stage_error: f64,
    pub accumulated_error: f64,
    pub backlash_error: f64,
    pub elastic_deformation_error: f64,
    pub wear_induced_error: f64,
    pub temperature_effect: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointingAccuracyResult {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub target_ra: f64,
    pub target_dec: f64,
    pub sky_zone: String,
    pub measured_ra: f64,
    pub measured_dec: f64,
    pub ra_error: f64,
    pub dec_error: f64,
    pub total_pointing_error: f64,
    pub error_azimuth_component: f64,
    pub error_elevation_component: f64,
    pub theoretical_precision: f64,
    pub achieved_precision: f64,
    pub error_transfer_coefficient: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmEvent {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    #[serde(default = "Uuid::new_v4")]
    pub alarm_id: Uuid,
    pub alarm_type: String,
    pub alarm_level: u8,
    pub alarm_message: String,
    pub affected_axis: Option<u8>,
    pub error_value: f64,
    pub threshold_value: f64,
    #[serde(default)]
    pub is_acknowledged: u8,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearStatus {
    pub timestamp: DateTime<Utc>,
    pub device_id: String,
    pub gear_id: u8,
    pub wear_level: f64,
    pub tooth_deflection: f64,
    pub lubrication_status: u8,
    pub vibration_amplitude: f64,
    pub rotation_speed: f64,
    pub torque: f64,
    pub estimated_life_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GearStage {
    pub stage_id: u8,
    pub teeth_input: u32,
    pub teeth_output: u32,
    pub theoretical_ratio: f64,
    pub backlash: f64,
    pub base_meshing_error: f64,
    pub wear_factor: f64,
    pub elastic_stiffness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisConfig {
    pub axis_id: u8,
    pub axis_name: String,
    pub gear_stages: Vec<GearStage>,
    pub bearing_clearance: f64,
    pub thermal_expansion_coeff: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub message_type: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse {
            success: true,
            message: "OK".to_string(),
            data: Some(data),
        }
    }

    pub fn error(message: &str) -> Self {
        ApiResponse {
            success: false,
            message: message.to_string(),
            data: None,
        }
    }
}
