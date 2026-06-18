use crate::models::{PointingAccuracyResult, SensorReading};
use chrono::Utc;

const DEG_TO_ARCMIN: f64 = 60.0;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

const SHAFT_YOUNG_MODULUS: f64 = 2.06e11;
const SHAFT_SHEAR_MODULUS: f64 = 7.93e10;
const SHAFT_DENSITY: f64 = 7850.0;
const SHAFT_DIAMETER: f64 = 0.025;
const SHAFT_LENGTH: f64 = 0.45;
const MODAL_DAMPING_RATIO: f64 = 0.035;
const OPERATING_SPEED_RPM: f64 = 2.5;

struct FlexibleAxisParams {
    torsion_stiffness: f64,
    bending_stiffness: f64,
    torsion_nat_freq: f64,
    bending_nat_freq: f64,
    mass_moment: f64,
}

pub struct PointingAnalyzer {
    systematic_errors: SystematicErrors,
    flex_params: FlexibleAxisParams,
}

struct SystematicErrors {
    zero_point_az: f64,
    zero_point_el: f64,
    axis_non_orthogonality: f64,
    tube_flexure_coeff: f64,
    collimation_error: f64,
    refraction_coeff: f64,
}

impl PointingAnalyzer {
    fn compute_flexible_params() -> FlexibleAxisParams {
        let r = SHAFT_DIAMETER / 2.0;
        let area = std::f64::consts::PI * r.powi(2);
        let i_polar = std::f64::consts::PI * r.powi(4) / 2.0;
        let i_area = std::f64::consts::PI * r.powi(4) / 4.0;

        let torsion_stiffness = SHAFT_SHEAR_MODULUS * i_polar / SHAFT_LENGTH;
        let bending_stiffness = SHAFT_YOUNG_MODULUS * i_area / SHAFT_LENGTH.powi(3) * 3.0;

        let shaft_mass = SHAFT_DENSITY * area * SHAFT_LENGTH;
        let mass_moment = shaft_mass * (3.0 * r.powi(2) + SHAFT_LENGTH.powi(2)) / 12.0;

        let end_mass_moment = 0.55;
        let total_moment = mass_moment + end_mass_moment;

        let torsion_nat_freq = (torsion_stiffness / total_moment).sqrt() / (2.0 * std::f64::consts::PI);
        let tip_equiv_mass = shaft_mass * 0.24 + 0.8;
        let bending_nat_freq = (3.0 * SHAFT_YOUNG_MODULUS * i_area
            / (tip_equiv_mass * SHAFT_LENGTH.powi(3))).sqrt()
            / (2.0 * std::f64::consts::PI);

        FlexibleAxisParams {
            torsion_stiffness,
            bending_stiffness,
            torsion_nat_freq,
            bending_nat_freq,
            mass_moment: total_moment,
        }
    }

    pub fn new() -> Self {
        PointingAnalyzer {
            systematic_errors: SystematicErrors {
                zero_point_az: 0.45,
                zero_point_el: 0.38,
                axis_non_orthogonality: 0.25,
                tube_flexure_coeff: 0.08,
                collimation_error: 0.32,
                refraction_coeff: 0.15,
            },
            flex_params: Self::compute_flexible_params(),
        }
    }

    pub fn determine_sky_zone(dec: f64, ra: f64) -> String {
        let _ra_normalized = if ra < 0.0 { ra + 360.0 } else { ra % 360.0 };
        
        if dec >= 60.0 {
            "北天极".to_string()
        } else if dec >= 30.0 {
            "北天区".to_string()
        } else if dec >= -30.0 {
            if dec.abs() <= 23.5 {
                "黄道带".to_string()
            } else {
                "赤道带".to_string()
            }
        } else if dec >= -60.0 {
            "南天区".to_string()
        } else {
            "南天极".to_string()
        }
    }

    pub fn atmospheric_refraction(elevation_deg: f64, temperature_c: f64, pressure_mb: f64) -> f64 {
        if elevation_deg <= 0.0 {
            return 0.0;
        }
        let el_rad = elevation_deg * std::f64::consts::PI / 180.0;
        let refraction_arcmin = 1.02 / el_rad.tan()
            * (pressure_mb / 1010.0)
            * (283.0 / (273.0 + temperature_c));
        refraction_arcmin.min(30.0)
    }

    pub fn tube_flexure(elevation_deg: f64, coeff: f64) -> f64 {
        let el_rad = elevation_deg * std::f64::consts::PI / 180.0;
        coeff * el_rad.cos()
    }

    fn dynamic_magnification_factor(r: f64, zeta: f64) -> f64 {
        let denom = ((1.0 - r.powi(2)).powi(2) + (2.0 * zeta * r).powi(2)).sqrt();
        1.0 / denom.max(0.01)
    }

    fn torsion_bending_coupling(el: f64) -> f64 {
        let el_rad = el * DEG_TO_RAD;
        1.0 + 0.35 * el_rad.sin().powi(2)
    }

    pub fn compute_error_transfer_coefficient(
        &self,
        az: f64,
        el: f64,
        cumulative_transmission_error: f64,
    ) -> f64 {
        let az_rad = az * DEG_TO_RAD;
        let el_rad = el * DEG_TO_RAD;

        let az_sensitivity = 1.0 / el_rad.cos().max(0.01);
        let el_sensitivity = 1.0;
        let geometric_factor = (az_sensitivity.powi(2) + el_sensitivity.powi(2)).sqrt();

        let omega_oper = OPERATING_SPEED_RPM * 2.0 * std::f64::consts::PI / 60.0;
        let omega_torsion = self.flex_params.torsion_nat_freq * 2.0 * std::f64::consts::PI;
        let omega_bending = self.flex_params.bending_nat_freq * 2.0 * std::f64::consts::PI;

        let r_t = omega_oper / omega_torsion;
        let r_b = omega_oper / omega_bending;

        let dmf_torsion = Self::dynamic_magnification_factor(r_t, MODAL_DAMPING_RATIO);
        let dmf_bending = Self::dynamic_magnification_factor(r_b, MODAL_DAMPING_RATIO);
        let dynamic_factor = (dmf_torsion.powi(2) + dmf_bending.powi(2) * 0.6).sqrt();

        let coupling_factor = Self::torsion_bending_coupling(el);

        let modes = [1.0, 2.8, 5.3];
        let damping = [MODAL_DAMPING_RATIO, MODAL_DAMPING_RATIO * 0.8, MODAL_DAMPING_RATIO * 0.6];
        let weights = [0.65, 0.25, 0.10];
        let mut modal_sum = 0.0;
        for i in 0..3 {
            let r_i = omega_oper / (modes[i] * omega_torsion);
            modal_sum += weights[i] * Self::dynamic_magnification_factor(r_i, damping[i]);
        }

        let wear_softening = 1.0 + cumulative_transmission_error * 0.08;

        geometric_factor * dynamic_factor * coupling_factor * modal_sum * wear_softening
    }

    pub fn equatorial_to_altaz(ra: f64, dec: f64, lst: f64, latitude: f64) -> (f64, f64) {
        let ra_rad = ra * std::f64::consts::PI / 180.0;
        let dec_rad = dec * std::f64::consts::PI / 180.0;
        let lst_rad = lst * std::f64::consts::PI / 180.0;
        let lat_rad = latitude * std::f64::consts::PI / 180.0;

        let ha = lst_rad - ra_rad;
        let sin_el = dec_rad.sin() * lat_rad.sin() + dec_rad.cos() * lat_rad.cos() * ha.cos();
        let el = sin_el.asin() * 180.0 / std::f64::consts::PI;

        let cos_az = (dec_rad.sin() - sin_el * lat_rad.sin()) / ((1.0 - sin_el.powi(2)).sqrt() * lat_rad.cos());
        let sin_az = -dec_rad.cos() * ha.sin();
        let mut az = sin_az.atan2(cos_az) * 180.0 / std::f64::consts::PI;
        if az < 0.0 {
            az += 360.0;
        }

        (az, el)
    }

    pub fn analyze_pointing_accuracy(
        &self,
        reading: &SensorReading,
        latitude: f64,
        lst: f64,
    ) -> PointingAccuracyResult {
        let target_ra = reading.theoretical_ra;
        let target_dec = reading.theoretical_dec;
        let measured_ra = reading.observed_star_ra;
        let measured_dec = reading.observed_star_dec;

        let ra_error = (measured_ra - target_ra) * DEG_TO_ARCMIN * (target_dec * std::f64::consts::PI / 180.0).cos();
        let dec_error = (measured_dec - target_dec) * DEG_TO_ARCMIN;

        let total_pointing_error = (ra_error.powi(2) + dec_error.powi(2)).sqrt();

        let (target_az, target_el) = Self::equatorial_to_altaz(target_ra, target_dec, lst, latitude);
        let (measured_az, measured_el) = Self::equatorial_to_altaz(measured_ra, measured_dec, lst, latitude);

        let error_az_comp = (measured_az - target_az) * DEG_TO_ARCMIN;
        let error_el_comp = (measured_el - target_el) * DEG_TO_ARCMIN;

        let transmission_component = reading.cumulative_transmission_error;
        let refraction = Self::atmospheric_refraction(target_el, reading.temperature, 1013.25);
        let flexure = Self::tube_flexure(target_el, self.systematic_errors.tube_flexure_coeff);

        let sys_ra = self.systematic_errors.zero_point_az
            + self.systematic_errors.axis_non_orthogonality * (target_dec * std::f64::consts::PI / 180.0).sin();
        let sys_dec = self.systematic_errors.zero_point_el + flexure;

        let theoretical_precision = ((transmission_component * 0.4).powi(2)
            + (self.systematic_errors.collimation_error).powi(2)
            + (refraction * 0.3).powi(2)
            + 0.05)
            .sqrt();

        let achieved_precision = total_pointing_error;

        let etc = self.compute_error_transfer_coefficient(
            target_az,
            target_el,
            reading.cumulative_transmission_error,
        );

        PointingAccuracyResult {
            timestamp: Utc::now(),
            device_id: reading.device_id.clone(),
            target_ra,
            target_dec,
            sky_zone: Self::determine_sky_zone(target_dec, target_ra),
            measured_ra,
            measured_dec,
            ra_error,
            dec_error,
            total_pointing_error,
            error_azimuth_component: error_az_comp,
            error_elevation_component: error_el_comp,
            theoretical_precision,
            achieved_precision,
            error_transfer_coefficient: etc,
        }
    }

    pub fn analyze_sky_zone_statistics(
        &self,
        results: &[PointingAccuracyResult],
    ) -> std::collections::HashMap<String, SkyZoneStats> {
        let mut zone_map: std::collections::HashMap<String, Vec<&PointingAccuracyResult>> = std::collections::HashMap::new();

        for r in results {
            zone_map.entry(r.sky_zone.clone()).or_default().push(r);
        }

        zone_map
            .into_iter()
            .map(|(zone, entries)| {
                let n = entries.len() as f64;
                let mean_err = entries.iter().map(|e| e.total_pointing_error).sum::<f64>() / n.max(1.0);
                let mean_ra = entries.iter().map(|e| e.ra_error.abs()).sum::<f64>() / n.max(1.0);
                let mean_dec = entries.iter().map(|e| e.dec_error.abs()).sum::<f64>() / n.max(1.0);
                let max_err = entries.iter().map(|e| e.total_pointing_error).fold(0.0f64, f64::max);
                let mean_etc = entries.iter().map(|e| e.error_transfer_coefficient).sum::<f64>() / n.max(1.0);

                (
                    zone,
                    SkyZoneStats {
                        sample_count: entries.len(),
                        mean_pointing_error: mean_err,
                        mean_ra_error: mean_ra,
                        mean_dec_error: mean_dec,
                        max_pointing_error: max_err,
                        mean_error_transfer_coeff: mean_etc,
                    },
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct SkyZoneStats {
    pub sample_count: usize,
    pub mean_pointing_error: f64,
    pub mean_ra_error: f64,
    pub mean_dec_error: f64,
    pub max_pointing_error: f64,
    pub mean_error_transfer_coeff: f64,
}

impl Default for PointingAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
