use crate::models::{AlarmEvent, SensorReading};
use chrono::Utc;
use std::sync::Arc;
use parking_lot::Mutex;

pub const CUMULATIVE_ERROR_WARNING_THRESHOLD: f64 = 0.8;
pub const CUMULATIVE_ERROR_ALARM_THRESHOLD: f64 = 1.0;
pub const GEAR_WEAR_WARNING_THRESHOLD: f64 = 0.6;
pub const GEAR_WEAR_ALARM_THRESHOLD: f64 = 0.85;

pub struct AlarmSystem {
    last_alarm_times: Arc<Mutex<std::collections::HashMap<String, chrono::DateTime<Utc>>>>,
    min_interval_seconds: i64,
}

impl AlarmSystem {
    pub fn new() -> Self {
        AlarmSystem {
            last_alarm_times: Arc::new(Mutex::new(std::collections::HashMap::new())),
            min_interval_seconds: 300,
        }
    }

    fn should_trigger_alarm(&self, alarm_key: &str) -> bool {
        let now = Utc::now();
        let mut last_times = self.last_alarm_times.lock();
        if let Some(last_time) = last_times.get(alarm_key) {
            if (now - *last_time).num_seconds() < self.min_interval_seconds {
                return false;
            }
        }
        last_times.insert(alarm_key.to_string(), now);
        true
    }

    pub fn check_cumulative_error(&self, reading: &SensorReading) -> Option<AlarmEvent> {
        let err = reading.cumulative_transmission_error;
        let key = format!("cumulative_error_{}", reading.device_id);

        if err >= CUMULATIVE_ERROR_ALARM_THRESHOLD {
            if !self.should_trigger_alarm(&key) {
                return None;
            }
            return Some(AlarmEvent {
                timestamp: Utc::now(),
                device_id: reading.device_id.clone(),
                alarm_id: uuid::Uuid::new_v4(),
                alarm_type: "累积误差超限".to_string(),
                alarm_level: 2,
                alarm_message: format!(
                    "浑仪{}累积传动误差达{:.3}角分，超过告警阈值1角分，请立即检查齿轮系统！",
                    reading.device_id, err
                ),
                affected_axis: None,
                error_value: err,
                threshold_value: CUMULATIVE_ERROR_ALARM_THRESHOLD,
                is_acknowledged: 0,
                acknowledged_at: None,
            });
        } else if err >= CUMULATIVE_ERROR_WARNING_THRESHOLD {
            if !self.should_trigger_alarm(&key) {
                return None;
            }
            return Some(AlarmEvent {
                timestamp: Utc::now(),
                device_id: reading.device_id.clone(),
                alarm_id: uuid::Uuid::new_v4(),
                alarm_type: "累积误差超限".to_string(),
                alarm_level: 1,
                alarm_message: format!(
                    "浑仪{}累积传动误差达{:.3}角分，接近告警阈值，请关注齿轮磨损情况。",
                    reading.device_id, err
                ),
                affected_axis: None,
                error_value: err,
                threshold_value: CUMULATIVE_ERROR_WARNING_THRESHOLD,
                is_acknowledged: 0,
                acknowledged_at: None,
            });
        }
        None
    }

    pub fn check_gear_wear(&self, reading: &SensorReading) -> Vec<AlarmEvent> {
        let mut alarms = Vec::new();
        let wears = [
            (reading.gear_wear_level_1, 1u8),
            (reading.gear_wear_level_2, 2u8),
            (reading.gear_wear_level_3, 3u8),
        ];

        for (wear, gear_id) in &wears {
            let key = format!("gear_wear_{}_{}", reading.device_id, gear_id);
            if *wear >= GEAR_WEAR_ALARM_THRESHOLD {
                if !self.should_trigger_alarm(&key) {
                    continue;
                }
                alarms.push(AlarmEvent {
                    timestamp: Utc::now(),
                    device_id: reading.device_id.clone(),
                    alarm_id: uuid::Uuid::new_v4(),
                    alarm_type: "齿轮磨损异常".to_string(),
                    alarm_level: 3,
                    alarm_message: format!(
                        "浑仪{}齿轮组{}磨损程度达{:.1}%，已严重磨损，建议立即停机更换！",
                        reading.device_id, gear_id, wear * 100.0
                    ),
                    affected_axis: Some(*gear_id),
                    error_value: *wear,
                    threshold_value: GEAR_WEAR_ALARM_THRESHOLD,
                    is_acknowledged: 0,
                    acknowledged_at: None,
                });
            } else if *wear >= GEAR_WEAR_WARNING_THRESHOLD {
                if !self.should_trigger_alarm(&key) {
                    continue;
                }
                alarms.push(AlarmEvent {
                    timestamp: Utc::now(),
                    device_id: reading.device_id.clone(),
                    alarm_id: uuid::Uuid::new_v4(),
                    alarm_type: "齿轮磨损异常".to_string(),
                    alarm_level: 1,
                    alarm_message: format!(
                        "浑仪{}齿轮组{}磨损程度达{:.1}%，建议安排维护检修。",
                        reading.device_id, gear_id, wear * 100.0
                    ),
                    affected_axis: Some(*gear_id),
                    error_value: *wear,
                    threshold_value: GEAR_WEAR_WARNING_THRESHOLD,
                    is_acknowledged: 0,
                    acknowledged_at: None,
                });
            }
        }
        alarms
    }

    pub fn process_reading(&self, reading: &SensorReading) -> Vec<AlarmEvent> {
        let mut alarms = Vec::new();

        if let Some(alarm) = self.check_cumulative_error(reading) {
            alarms.push(alarm);
        }

        alarms.extend(self.check_gear_wear(reading));

        alarms
    }
}

impl Default for AlarmSystem {
    fn default() -> Self {
        Self::new()
    }
}
