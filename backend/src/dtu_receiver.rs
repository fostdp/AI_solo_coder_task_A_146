use crate::clickhouse::ClickHouseClient;
use crate::models::{GearParamsConfig, HunyiError, PipelineChannels, PipelineMessage, SensorReading, validate_reading};
use std::sync::Arc;

pub struct DtuReceiver {
    ch_client: Arc<ClickHouseClient>,
    gear_cfg: Arc<GearParamsConfig>,
    channels: PipelineChannels,
}

impl DtuReceiver {
    pub fn new(
        ch_client: Arc<ClickHouseClient>,
        gear_cfg: Arc<GearParamsConfig>,
        channels: PipelineChannels,
    ) -> Self {
        DtuReceiver { ch_client, gear_cfg, channels }
    }

    pub fn compute_cumulative_error(&self, r: &SensorReading) -> f64 {
        let gear_errors = [
            r.gear_meshing_error_1,
            r.gear_meshing_error_2,
            r.gear_meshing_error_3,
        ];
        let bearing_errors = [
            r.bearing_clearance_1,
            r.bearing_clearance_2,
            r.bearing_clearance_3,
        ];
        let wear_levels = [
            r.gear_wear_level_1,
            r.gear_wear_level_2,
            r.gear_wear_level_3,
        ];

        let mut cumulative = 0.0;
        for i in 0..3 {
            cumulative += gear_errors[i] * (1.0 + wear_levels[i] * 2.0);
            cumulative += bearing_errors[i] * 0.5;
        }
        cumulative += (r.temperature - 20.0).abs() * 0.02;
        cumulative
    }

    pub async fn ingest(&self, mut reading: SensorReading) -> Result<Arc<SensorReading>, HunyiError> {
        validate_reading(&reading, &self.gear_cfg.validation)?;

        reading.cumulative_transmission_error = self.compute_cumulative_error(&reading);

        self.ch_client
            .insert_sensor_reading(&reading)
            .await
            .map_err(|e| HunyiError::ClickHouse(e.to_string()))?;

        let arc = Arc::new(reading);

        if let Err(e) = self.channels.to_transmission.send(PipelineMessage::ValidatedReading(arc.clone())).await {
            log::error!("DTU->transmission channel error: {}", e);
        }
        if let Err(e) = self.channels.to_pointing.send(PipelineMessage::ValidatedReading(arc.clone())).await {
            log::error!("DTU->pointing channel error: {}", e);
        }
        if let Err(e) = self.channels.to_alarm_ws.send(PipelineMessage::ValidatedReading(arc.clone())).await {
            log::error!("DTU->alarm_ws channel error: {}", e);
        }

        log::debug!(
            "DTU ingested reading device={}, cum_err={:.3} arcmin",
            arc.device_id, arc.cumulative_transmission_error
        );
        Ok(arc)
    }

    pub fn validation_rules(&self) -> &crate::models::ValidationRanges {
        &self.gear_cfg.validation
    }
}
