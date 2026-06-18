CREATE DATABASE IF NOT EXISTS hunyi_astronomy
    COMMENT '古代浑仪机械传动误差仿真与天体指向精度分析数据库'
    ENGINE = Atomic;

USE hunyi_astronomy;

CREATE TABLE IF NOT EXISTS sensor_readings (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    device_id String COMMENT '浑仪设备编号',
    axis_azimuth_angle Float64 COMMENT '方位轴转角(度)',
    axis_elevation_angle Float64 COMMENT '赤纬轴转角(度)',
    axis_equatorial_angle Float64 COMMENT '赤道轴转角(度)',
    gear_meshing_error_1 Float64 COMMENT '齿轮组1啮合误差(角分)',
    gear_meshing_error_2 Float64 COMMENT '齿轮组2啮合误差(角分)',
    gear_meshing_error_3 Float64 COMMENT '齿轮组3啮合误差(角分)',
    bearing_clearance_1 Float64 COMMENT '轴承1间隙(角分)',
    bearing_clearance_2 Float64 COMMENT '轴承2间隙(角分)',
    bearing_clearance_3 Float64 COMMENT '轴承3间隙(角分)',
    observed_star_ra Float64 COMMENT '观测星体赤经(度)',
    observed_star_dec Float64 COMMENT '观测星体赤纬(度)',
    theoretical_ra Float64 COMMENT '理论赤经(度)',
    theoretical_dec Float64 COMMENT '理论赤纬(度)',
    ra_deviation Float64 COMMENT '赤经偏差(角分)',
    dec_deviation Float64 COMMENT '赤纬偏差(角分)',
    cumulative_transmission_error Float64 COMMENT '累积传动误差(角分)',
    gear_wear_level_1 Float64 COMMENT '齿轮1磨损程度(0-1)',
    gear_wear_level_2 Float64 COMMENT '齿轮2磨损程度(0-1)',
    gear_wear_level_3 Float64 COMMENT '齿轮3磨损程度(0-1)',
    temperature Float64 COMMENT '环境温度(摄氏度)',
    humidity Float64 COMMENT '环境湿度(%)'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '浑仪传感器原始读数表';

CREATE TABLE IF NOT EXISTS transmission_error_analysis (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    device_id String,
    axis_id UInt8 COMMENT '轴ID:1=方位,2=赤纬,3=赤道',
    input_angle Float64 COMMENT '输入轴角度(度)',
    output_angle Float64 COMMENT '输出轴角度(度)',
    theoretical_ratio Float64 COMMENT '理论传动比',
    actual_ratio Float64 COMMENT '实际传动比',
    single_stage_error Float64 COMMENT '单级传动误差(角分)',
    accumulated_error Float64 COMMENT '累积传动误差(角分)',
    backlash_error Float64 COMMENT '齿隙误差(角分)',
    elastic_deformation_error Float64 COMMENT '弹性变形误差(角分)',
    wear_induced_error Float64 COMMENT '磨损引起误差(角分)',
    temperature_effect Float64 COMMENT '温度效应(角分)'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, axis_id, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '传动误差分析结果表';

CREATE TABLE IF NOT EXISTS pointing_accuracy_analysis (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    device_id String,
    target_ra Float64 COMMENT '目标赤经(度)',
    target_dec Float64 COMMENT '目标赤纬(度)',
    sky_zone String COMMENT '天区:北天极/赤道带/南天极/黄道带',
    measured_ra Float64 COMMENT '实测赤经(度)',
    measured_dec Float64 COMMENT '实测赤纬(度)',
    ra_error Float64 COMMENT '赤经指向误差(角分)',
    dec_error Float64 COMMENT '赤纬指向误差(角分)',
    total_pointing_error Float64 COMMENT '总指向误差(角分)',
    error_azimuth_component Float64 COMMENT '方位误差分量(角分)',
    error_elevation_component Float64 COMMENT '高度误差分量(角分)',
    theoretical_precision Float64 COMMENT '理论精度(角分)',
    achieved_precision Float64 COMMENT '实际达到精度(角分)',
    error_transfer_coefficient Float64 COMMENT '误差传递系数'
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, sky_zone, timestamp)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '指向精度分析结果表';

CREATE TABLE IF NOT EXISTS alarm_events (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    device_id String,
    alarm_id UUID DEFAULT generateUUIDv4(),
    alarm_type String COMMENT '告警类型:累积误差超限/齿轮磨损异常/传感器故障',
    alarm_level UInt8 COMMENT '告警级别:1=预警,2=告警,3=严重',
    alarm_message String,
    affected_axis Nullable(UInt8),
    error_value Float64 COMMENT '触发告警的误差值(角分)',
    threshold_value Float64 COMMENT '告警阈值(角分)',
    is_acknowledged UInt8 DEFAULT 0,
    acknowledged_at Nullable(DateTime64(3, 'Asia/Shanghai'))
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, alarm_level, timestamp)
TTL timestamp + INTERVAL 3 YEAR
COMMENT '告警事件表';

CREATE TABLE IF NOT EXISTS gear_status (
    timestamp DateTime64(3, 'Asia/Shanghai') DEFAULT now64(3),
    device_id String,
    gear_id UInt8 COMMENT '齿轮ID:1-6',
    wear_level Float64 COMMENT '磨损程度:0=new,1=完全磨损',
    tooth_deflection Float64 COMMENT '齿面挠度(微米)',
    lubrication_status UInt8 COMMENT '润滑状态:0=良好,1=需注意,2=需加油',
    vibration_amplitude Float64 COMMENT '振动幅值(微米)',
    rotation_speed Float64 COMMENT '转速(转/分)',
    torque Float64 COMMENT '扭矩(N·m)',
    estimated_life_hours Float64 COMMENT '预估剩余寿命(小时)'
)
ENGINE = ReplacingMergeTree(timestamp)
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, gear_id)
TTL timestamp + INTERVAL 1 YEAR
COMMENT '齿轮状态表(最新状态)';

CREATE MATERIALIZED VIEW IF NOT EXISTS sensor_readings_1min_mv
ENGINE = SummingMergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (device_id, toStartOfMinute(timestamp))
AS SELECT
    toStartOfMinute(timestamp) AS timestamp,
    device_id,
    count() AS readings_count,
    avg(cumulative_transmission_error) AS avg_cumulative_error,
    max(cumulative_transmission_error) AS max_cumulative_error,
    avg(ra_deviation) AS avg_ra_deviation,
    avg(dec_deviation) AS avg_dec_deviation,
    avg(gear_meshing_error_1 + gear_meshing_error_2 + gear_meshing_error_3) / 3 AS avg_gear_error,
    avg(gear_wear_level_1) AS avg_wear_1,
    avg(gear_wear_level_2) AS avg_wear_2,
    avg(gear_wear_level_3) AS avg_wear_3
FROM sensor_readings
GROUP BY device_id, toStartOfMinute(timestamp);

CREATE TABLE IF NOT EXISTS sky_zone_reference (
    zone_id UInt8,
    zone_name String,
    ra_min Float64,
    ra_max Float64,
    dec_min Float64,
    dec_max Float64,
    description String
)
ENGINE = ReplacingMergeTree()
ORDER BY zone_id;

INSERT INTO sky_zone_reference VALUES
(1, '北天极', 0, 360, 60, 90, '北天极附近区域，赤纬60度以上'),
(2, '北天区', 0, 360, 30, 60, '北天中纬度区域'),
(3, '赤道带', 0, 360, -30, 30, '天赤道附近区域'),
(4, '南天区', 0, 360, -60, -30, '南天中纬度区域'),
(5, '南天极', 0, 360, -90, -60, '南天极附近区域'),
(6, '黄道带', 0, 360, -23.5, 23.5, '黄道附近区域，行星观测带');
