use crate::models::{AxisConfig, GearStage, TransmissionErrorResult, SensorReading};
use chrono::Utc;
use rand::Rng;

const DEG_TO_ARCSEC: f64 = 3600.0;
const DEG_TO_ARCMIN: f64 = 60.0;
const ARCMIN_TO_RAD: f64 = std::f64::consts::PI / (180.0 * 60.0);

pub struct TransmissionSimulator {
    axes: Vec<AxisConfig>,
}

impl TransmissionSimulator {
    pub fn new() -> Self {
        TransmissionSimulator {
            axes: Self::default_axes(),
        }
    }

    fn default_axes() -> Vec<AxisConfig> {
        vec![
            AxisConfig {
                axis_id: 1,
                axis_name: "方位轴".to_string(),
                gear_stages: vec![
                    GearStage {
                        stage_id: 1,
                        teeth_input: 20,
                        teeth_output: 120,
                        theoretical_ratio: 6.0,
                        backlash: 0.8,
                        base_meshing_error: 0.15,
                        wear_factor: 0.0,
                        elastic_stiffness: 5.0e6,
                    },
                    GearStage {
                        stage_id: 2,
                        teeth_input: 25,
                        teeth_output: 180,
                        theoretical_ratio: 7.2,
                        backlash: 0.6,
                        base_meshing_error: 0.12,
                        wear_factor: 0.0,
                        elastic_stiffness: 4.5e6,
                    },
                ],
                bearing_clearance: 0.3,
                thermal_expansion_coeff: 1.2e-5,
            },
            AxisConfig {
                axis_id: 2,
                axis_name: "赤纬轴".to_string(),
                gear_stages: vec![
                    GearStage {
                        stage_id: 1,
                        teeth_input: 18,
                        teeth_output: 144,
                        theoretical_ratio: 8.0,
                        backlash: 0.9,
                        base_meshing_error: 0.18,
                        wear_factor: 0.0,
                        elastic_stiffness: 4.8e6,
                    },
                    GearStage {
                        stage_id: 2,
                        teeth_input: 22,
                        teeth_output: 176,
                        theoretical_ratio: 8.0,
                        backlash: 0.7,
                        base_meshing_error: 0.14,
                        wear_factor: 0.0,
                        elastic_stiffness: 4.2e6,
                    },
                ],
                bearing_clearance: 0.35,
                thermal_expansion_coeff: 1.15e-5,
            },
            AxisConfig {
                axis_id: 3,
                axis_name: "赤道轴".to_string(),
                gear_stages: vec![
                    GearStage {
                        stage_id: 1,
                        teeth_input: 24,
                        teeth_output: 144,
                        theoretical_ratio: 6.0,
                        backlash: 0.7,
                        base_meshing_error: 0.13,
                        wear_factor: 0.0,
                        elastic_stiffness: 5.2e6,
                    },
                    GearStage {
                        stage_id: 2,
                        teeth_input: 20,
                        teeth_output: 200,
                        theoretical_ratio: 10.0,
                        backlash: 0.85,
                        base_meshing_error: 0.16,
                        wear_factor: 0.0,
                        elastic_stiffness: 4.0e6,
                    },
                ],
                bearing_clearance: 0.28,
                thermal_expansion_coeff: 1.25e-5,
            },
        ]
    }

    pub fn get_axis_config(&self, axis_id: u8) -> Option<&AxisConfig> {
        self.axes.iter().find(|a| a.axis_id == axis_id)
    }

    pub fn simulate_single_stage(
        &self,
        stage: &GearStage,
        input_angle: f64,
        rotation_direction: i32,
        wear_level: f64,
        temperature: f64,
        torque: f64,
    ) -> (f64, f64, f64, f64, f64, f64) {
        let mut rng = rand::thread_rng();

        let theoretical_output = input_angle * stage.theoretical_ratio;

        let wear_multiplier = 1.0 + wear_level * 3.0;
        let dynamic_meshing_error = stage.base_meshing_error
            * wear_multiplier
            * (1.0 + 0.3 * (input_angle * 2.0 * std::f64::consts::PI / 360.0).sin())
            + rng.gen_range(-0.05..0.05);

        let direction_factor = if rotation_direction != 0 {
            (rotation_direction as f64).signum()
        } else {
            0.0
        };
        let backlash_contribution = if direction_factor != 0.0 {
            stage.backlash * (1.0 + wear_level * 2.0) * 0.5 * (1.0 + direction_factor)
                + rng.gen_range(-0.05..0.05) * (1.0 + wear_level)
        } else {
            0.0
        };

        let elastic_deflection = (torque * 1000.0 / stage.elastic_stiffness) * DEG_TO_ARCMIN
            * (1.0 + wear_level * 1.5);

        let temp_effect = (temperature - 20.0) * 8.5e-4 * DEG_TO_ARCMIN
            * (1.0 + wear_level * 0.5);

        let total_single_error = dynamic_meshing_error
            + backlash_contribution.abs()
            + elastic_deflection
            + temp_effect.abs();

        let noise = rng.gen_range(-0.03..0.03);
        let actual_output = theoretical_output - total_single_error / DEG_TO_ARCMIN + noise / DEG_TO_ARCMIN;
        let actual_ratio = if input_angle.abs() > 1e-10 {
            actual_output / input_angle
        } else {
            stage.theoretical_ratio
        };

        (
            theoretical_output,
            actual_output,
            actual_ratio,
            dynamic_meshing_error,
            backlash_contribution.abs(),
            elastic_deflection,
        )
    }

    pub fn simulate_axis_transmission(
        &self,
        axis: &AxisConfig,
        input_angle: f64,
        rotation_direction: i32,
        wear_levels: &[f64],
        temperature: f64,
        torque: f64,
    ) -> TransmissionErrorResult {
        let mut accumulated_error = 0.0;
        let mut current_input = input_angle;
        let mut total_backlash = 0.0;
        let mut total_elastic = 0.0;
        let mut total_wear_error = 0.0;
        let mut total_temp_effect = 0.0;
        let mut theoretical_total_ratio = 1.0;

        for (idx, stage) in axis.gear_stages.iter().enumerate() {
            let wear_lvl = wear_levels.get(idx).copied().unwrap_or(0.0);
            theoretical_total_ratio *= stage.theoretical_ratio;

            let (_, actual_output, _, meshing_err, backlash, elastic) =
                self.simulate_single_stage(stage, current_input, rotation_direction, wear_lvl, temperature, torque);

            accumulated_error += meshing_err + backlash + elastic;
            total_backlash += backlash;
            total_elastic += elastic;
            total_wear_error += meshing_err * wear_lvl * 2.0;
            total_temp_effect += (temperature - 20.0) * 8.5e-4 * DEG_TO_ARCMIN * (1.0 + wear_lvl * 0.5);

            current_input = actual_output;
        }

        accumulated_error += axis.bearing_clearance * (1.0 + wear_levels.first().copied().unwrap_or(0.0));

        let final_output = current_input;
        let actual_total_ratio = if input_angle.abs() > 1e-10 {
            final_output / input_angle
        } else {
            theoretical_total_ratio
        };

        TransmissionErrorResult {
            timestamp: Utc::now(),
            device_id: "HUNYI-001".to_string(),
            axis_id: axis.axis_id,
            input_angle,
            output_angle: final_output,
            theoretical_ratio: theoretical_total_ratio,
            actual_ratio: actual_total_ratio,
            single_stage_error: accumulated_error / axis.gear_stages.len() as f64,
            accumulated_error,
            backlash_error: total_backlash,
            elastic_deformation_error: total_elastic,
            wear_induced_error: total_wear_error,
            temperature_effect: total_temp_effect,
        }
    }

    pub fn compute_cumulative_error(&self, readings: &SensorReading) -> f64 {
        let gear_errors = [
            readings.gear_meshing_error_1,
            readings.gear_meshing_error_2,
            readings.gear_meshing_error_3,
        ];
        let bearing_errors = [
            readings.bearing_clearance_1,
            readings.bearing_clearance_2,
            readings.bearing_clearance_3,
        ];
        let wear_levels = [
            readings.gear_wear_level_1,
            readings.gear_wear_level_2,
            readings.gear_wear_level_3,
        ];

        let mut cumulative = 0.0;
        for i in 0..3 {
            cumulative += gear_errors[i] * (1.0 + wear_levels[i] * 2.0);
            cumulative += bearing_errors[i] * 0.5;
        }

        let temp_correction = (readings.temperature - 20.0).abs() * 0.02;
        cumulative += temp_correction;

        cumulative
    }

    pub fn simulate_backlash_collision(
        &self,
        stage: &GearStage,
        angular_velocity: f64,
        direction_change: bool,
        wear_level: f64,
    ) -> (f64, f64, f64) {
        let mut rng = rand::thread_rng();

        let effective_backlash = stage.backlash * (1.0 + wear_level * 2.5);
        let impact_velocity = angular_velocity * ARCMIN_TO_RAD;

        let collision_force = if direction_change && angular_velocity.abs() > 0.01 {
            let equivalent_mass = 0.5;
            let impact_duration = 1.0e-4;
            equivalent_mass * impact_velocity / impact_duration
        } else {
            0.0
        };

        let impact_error = if direction_change {
            effective_backlash * (0.6 + 0.4 * rng.gen::<f64>())
        } else {
            0.0
        };

        let vibration_decay = (-impact_velocity * 5.0).exp();
        let residual_vibration = impact_error * (1.0 - vibration_decay);

        (impact_error, collision_force, residual_vibration)
    }
}

impl Default for TransmissionSimulator {
    fn default() -> Self {
        Self::new()
    }
}
