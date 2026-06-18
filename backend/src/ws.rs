use crate::models::{AlarmEvent, PointingAccuracyResult, SensorReading, TransmissionErrorResult, WebSocketMessage};
use actix::prelude::*;
use std::collections::HashMap;
use chrono::Utc;

pub struct WsBroadcastServer {
    sessions: HashMap<usize, Recipient<WsMessage>>,
    counter: usize,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsMessage(pub String);

#[derive(Message)]
#[rtype(usize)]
pub struct Connect {
    pub addr: Recipient<WsMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub id: usize,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastSensorReading {
    pub reading: SensorReading,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastTransmissionError {
    pub result: TransmissionErrorResult,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastPointingAccuracy {
    pub result: PointingAccuracyResult,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastAlarm {
    pub alarm: AlarmEvent,
}

impl WsBroadcastServer {
    pub fn new() -> Self {
        WsBroadcastServer {
            sessions: HashMap::new(),
            counter: 0,
        }
    }

    fn broadcast(&self, msg: &str) {
        for (_, addr) in &self.sessions {
            let _ = addr.do_send(WsMessage(msg.to_string()));
        }
    }
}

impl Actor for WsBroadcastServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for WsBroadcastServer {
    type Result = usize;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        self.counter += 1;
        self.sessions.insert(self.counter, msg.addr);
        self.counter
    }
}

impl Handler<Disconnect> for WsBroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.sessions.remove(&msg.id);
    }
}

impl Handler<BroadcastSensorReading> for WsBroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: BroadcastSensorReading, _: &mut Context<Self>) {
        let ws_msg = WebSocketMessage {
            message_type: "sensor_reading".to_string(),
            payload: serde_json::to_value(&msg.reading).unwrap_or_default(),
            timestamp: Utc::now(),
        };
        if let Ok(json) = serde_json::to_string(&ws_msg) {
            self.broadcast(&json);
        }
    }
}

impl Handler<BroadcastTransmissionError> for WsBroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: BroadcastTransmissionError, _: &mut Context<Self>) {
        let ws_msg = WebSocketMessage {
            message_type: "transmission_error".to_string(),
            payload: serde_json::to_value(&msg.result).unwrap_or_default(),
            timestamp: Utc::now(),
        };
        if let Ok(json) = serde_json::to_string(&ws_msg) {
            self.broadcast(&json);
        }
    }
}

impl Handler<BroadcastPointingAccuracy> for WsBroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: BroadcastPointingAccuracy, _: &mut Context<Self>) {
        let ws_msg = WebSocketMessage {
            message_type: "pointing_accuracy".to_string(),
            payload: serde_json::to_value(&msg.result).unwrap_or_default(),
            timestamp: Utc::now(),
        };
        if let Ok(json) = serde_json::to_string(&ws_msg) {
            self.broadcast(&json);
        }
    }
}

impl Handler<BroadcastAlarm> for WsBroadcastServer {
    type Result = ();

    fn handle(&mut self, msg: BroadcastAlarm, _: &mut Context<Self>) {
        let ws_msg = WebSocketMessage {
            message_type: "alarm".to_string(),
            payload: serde_json::to_value(&msg.alarm).unwrap_or_default(),
            timestamp: Utc::now(),
        };
        if let Ok(json) = serde_json::to_string(&ws_msg) {
            self.broadcast(&json);
        }
    }
}

impl Default for WsBroadcastServer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WsSession {
    pub id: usize,
    pub addr: Addr<WsBroadcastServer>,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.addr
            .send(Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(id) => act.id = id,
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        self.addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

impl Handler<WsMessage> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Pong(_)) => (),
            Ok(ws::Message::Text(_text)) => (),
            Ok(ws::Message::Binary(_bin)) => (),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Nop) => (),
            Err(_) => ctx.stop(),
        }
    }
}
