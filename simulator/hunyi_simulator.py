#!/usr/bin/env python3
"""
古代浑仪机械传动误差仿真 - 传感器模拟器
模拟宋代浑仪各轴传感器每分钟上报数据
"""

import json
import math
import random
import time
import datetime
import requests
import argparse
from typing import Dict, Any

DEG_TO_RAD = math.pi / 180.0
DEG_TO_ARCMIN = 60.0


class HunyiSensorSimulator:
    def __init__(self, device_id: str = "HUNYI-001", api_url: str = "http://localhost:8080"):
        self.device_id = device_id
        self.api_url = api_url
        self.endpoint = f"{api_url}/api/sensor"

        self.azimuth_angle = 0.0
        self.elevation_angle = 45.0
        self.equatorial_angle = 0.0

        self.azimuth_speed = 0.15
        self.elevation_speed = 0.02
        self.equatorial_speed = 0.25

        self.gear_wear_1 = random.uniform(0.05, 0.15)
        self.gear_wear_2 = random.uniform(0.03, 0.12)
        self.gear_wear_3 = random.uniform(0.04, 0.14)

        self.base_temperature = 22.0
        self.base_humidity = 55.0

        self.tick_count = 0

    def _generate_angles(self) -> tuple:
        self.azimuth_angle = (self.azimuth_angle + self.azimuth_speed + random.uniform(-0.02, 0.02)) % 360.0
        self.elevation_angle = max(
            5.0,
            min(85.0, self.elevation_angle + random.uniform(-0.05, 0.05) + self.elevation_speed * math.sin(self.tick_count * 0.01))
        )
        self.equatorial_angle = (self.equatorial_angle + self.equatorial_speed + random.uniform(-0.03, 0.03)) % 360.0

        return self.azimuth_angle, self.elevation_angle, self.equatorial_angle

    def _generate_gear_meshing_errors(self) -> tuple:
        base_errors = [0.12, 0.10, 0.11]
        wears = [self.gear_wear_1, self.gear_wear_2, self.gear_wear_3]
        errors = []
        for i in range(3):
            base = base_errors[i]
            wear_multiplier = 1.0 + wears[i] * 3.0
            oscillation = 0.05 * math.sin(self.tick_count * 0.1 + i * 2.0)
            noise = random.gauss(0, 0.02)
            error = base * wear_multiplier + oscillation + noise
            errors.append(max(0.0, error))
        return tuple(errors)

    def _generate_bearing_clearances(self) -> tuple:
        bases = [0.15, 0.12, 0.14]
        clearances = []
        for i in range(3):
            temp_effect = (self.base_temperature - 20.0) * 0.008
            wear_effect = [self.gear_wear_1, self.gear_wear_2, self.gear_wear_3][i] * 0.3
            noise = random.gauss(0, 0.015)
            c = bases[i] + temp_effect + wear_effect + noise
            clearances.append(max(0.0, c))
        return tuple(clearances)

    def _generate_star_position(self) -> tuple:
        ra_target = random.uniform(0, 360)
        dec_target = random.uniform(-70, 70)

        total_gear_error = sum([
            self.gear_wear_1 * 0.3,
            self.gear_wear_2 * 0.3,
            self.gear_wear_3 * 0.3,
        ])

        ra_error_arcmin = random.gauss(0.3 + total_gear_error * 5.0, 0.15 + total_gear_error)
        dec_error_arcmin = random.gauss(0.25 + total_gear_error * 4.0, 0.12 + total_gear_error * 0.8)

        dec_cos = max(0.01, math.cos(dec_target * DEG_TO_RAD))
        ra_observed = ra_target + ra_error_arcmin / DEG_TO_ARCMIN / dec_cos
        dec_observed = dec_target + dec_error_arcmin / DEG_TO_ARCMIN

        ra_observed = ra_observed % 360
        dec_observed = max(-90, min(90, dec_observed))

        return (
            ra_observed, dec_observed,
            ra_target, dec_target,
            ra_error_arcmin, dec_error_arcmin
        )

    def _compute_cumulative_error(self, gear_errors, bearing_clearances) -> float:
        cumulative = 0.0
        wears = [self.gear_wear_1, self.gear_wear_2, self.gear_wear_3]
        for i in range(3):
            cumulative += gear_errors[i] * (1.0 + wears[i] * 2.0)
            cumulative += bearing_clearances[i] * 0.5
        cumulative += (self.base_temperature - 20.0) * 0.02
        return cumulative

    def _update_wear_levels(self):
        self.gear_wear_1 = min(0.99, self.gear_wear_1 + random.uniform(0, 0.00005))
        self.gear_wear_2 = min(0.99, self.gear_wear_2 + random.uniform(0, 0.00004))
        self.gear_wear_3 = min(0.99, self.gear_wear_3 + random.uniform(0, 0.000045))

        if self.tick_count % 1440 == 0 and random.random() < 0.05:
            spike = random.uniform(0.02, 0.08)
            self.gear_wear_1 = min(0.99, self.gear_wear_1 + spike)
            print(f"[WARN] Gear 1 wear spike detected, current wear: {self.gear_wear_1:.4f}")

    def generate_reading(self) -> Dict[str, Any]:
        self.tick_count += 1

        azimuth, elevation, equatorial = self._generate_angles()
        gear_err_1, gear_err_2, gear_err_3 = self._generate_gear_meshing_errors()
        brg_clear_1, brg_clear_2, brg_clear_3 = self._generate_bearing_clearances()
        obs_ra, obs_dec, theo_ra, theo_dec, ra_dev, dec_dev = self._generate_star_position()

        temp = self.base_temperature + random.gauss(0, 0.8) + 2.0 * math.sin(self.tick_count / 720.0)
        humidity = max(20.0, min(95.0, self.base_humidity + random.gauss(0, 3.0) - 10.0 * math.sin(self.tick_count / 720.0)))

        cumulative = self._compute_cumulative_error(
            [gear_err_1, gear_err_2, gear_err_3],
            [brg_clear_1, brg_clear_2, brg_clear_3]
        )

        self._update_wear_levels()

        return {
            "timestamp": datetime.datetime.utcnow().isoformat() + "Z",
            "device_id": self.device_id,
            "axis_azimuth_angle": round(azimuth, 6),
            "axis_elevation_angle": round(elevation, 6),
            "axis_equatorial_angle": round(equatorial, 6),
            "gear_meshing_error_1": round(gear_err_1, 6),
            "gear_meshing_error_2": round(gear_err_2, 6),
            "gear_meshing_error_3": round(gear_err_3, 6),
            "bearing_clearance_1": round(brg_clear_1, 6),
            "bearing_clearance_2": round(brg_clear_2, 6),
            "bearing_clearance_3": round(brg_clear_3, 6),
            "observed_star_ra": round(obs_ra, 6),
            "observed_star_dec": round(obs_dec, 6),
            "theoretical_ra": round(theo_ra, 6),
            "theoretical_dec": round(theo_dec, 6),
            "ra_deviation": round(ra_dev, 6),
            "dec_deviation": round(dec_dev, 6),
            "cumulative_transmission_error": round(cumulative, 6),
            "gear_wear_level_1": round(self.gear_wear_1, 6),
            "gear_wear_level_2": round(self.gear_wear_2, 6),
            "gear_wear_level_3": round(self.gear_wear_3, 6),
            "temperature": round(temp, 3),
            "humidity": round(humidity, 3),
        }

    def send_reading(self, reading: Dict[str, Any]) -> bool:
        try:
            resp = requests.post(
                self.endpoint,
                json=reading,
                headers={"Content-Type": "application/json"},
                timeout=10
            )
            if resp.status_code == 200:
                data = resp.json()
                alarms = data.get("data", {}).get("alarms", [])
                if alarms:
                    for alarm in alarms:
                        lvl = alarm.get("alarm_level", 0)
                        prefix = "[ALARM]" if lvl >= 2 else "[WARN]"
                        print(f"{prefix} {alarm.get('alarm_message', '')}")
                return True
            else:
                print(f"[ERROR] HTTP {resp.status_code}: {resp.text[:200]}")
                return False
        except Exception as e:
            print(f"[ERROR] Failed to send reading: {e}")
            return False

    def run(self, interval_seconds: int = 60, max_iterations: int = -1):
        print(f"Starting Hunyi sensor simulator: device={self.device_id}")
        print(f"  API endpoint: {self.endpoint}")
        print(f"  Report interval: {interval_seconds}s")
        print(f"  Initial gear wear levels: 1={self.gear_wear_1:.4f}, 2={self.gear_wear_2:.4f}, 3={self.gear_wear_3:.4f}")
        print("=" * 70)

        count = 0
        try:
            while True:
                reading = self.generate_reading()
                ts = reading["timestamp"]
                cumulative = reading["cumulative_transmission_error"]
                ra_dev = reading["ra_deviation"]
                dec_dev = reading["dec_deviation"]
                status = self.send_reading(reading)
                status_str = "OK" if status else "FAIL"

                print(
                    f"[{ts[:19]}] #{count} err={cumulative:.3f}' "
                    f"ΔRA={ra_dev:+.3f}' ΔDec={dec_dev:+.3f}' "
                    f"gear1={reading['gear_wear_level_1']:.3f} -> {status_str}"
                )

                count += 1
                if max_iterations > 0 and count >= max_iterations:
                    print(f"Reached max iterations ({max_iterations}), stopping.")
                    break

                time.sleep(interval_seconds)

        except KeyboardInterrupt:
            print("\nSimulator stopped by user.")
            print(f"Final gear wear: 1={self.gear_wear_1:.4f}, 2={self.gear_wear_2:.4f}, 3={self.gear_wear_3:.4f}")


def main():
    parser = argparse.ArgumentParser(description="浑仪传感器模拟器")
    parser.add_argument("--device", default="HUNYI-001", help="设备ID")
    parser.add_argument("--api", default="http://localhost:8080", help="后端API地址")
    parser.add_argument("--interval", type=int, default=60, help="上报间隔（秒）")
    parser.add_argument("--count", type=int, default=-1, help="最大上报次数，-1为无限")
    parser.add_argument("--fast", action="store_true", help="快速模式：1秒间隔")
    args = parser.parse_args()

    interval = 1 if args.fast else args.interval

    sim = HunyiSensorSimulator(device_id=args.device, api_url=args.api)
    sim.run(interval_seconds=interval, max_iterations=args.count)


if __name__ == "__main__":
    main()
