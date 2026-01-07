use crate::{Conn, Message, MqttEvent, State, handler::IHandler};
use std::time::Duration;
use tokio::sync::watch;
pub(crate) struct Manager {
    state: watch::Sender<State>,
    handler: Box<dyn IHandler>,
    conn: Conn,
}

pub(crate) enum MqttEventData {
    Error(String),
    Connected,
    Disconnected,
    IncomeMsg(Message),
}

impl Manager {
    pub(crate) fn new(state: watch::Sender<State>, conn: Conn, handler: Box<dyn IHandler>) -> Self {
        Manager {
            state,
            handler,
            conn,
        }
    }
    pub(crate) fn run(mut self) {
        tokio::spawn(async move {
            loop {
                let s = self.conn.poll_msg().await;
                let Some(s) = s else {
                    continue;
                };
                match s {
                    MqttEventData::Disconnected => {
                        if *self.state.borrow() != State::Disconnected {
                            self.state.send(State::Disconnected).ok();
                            self.handler.on_event(MqttEvent::Disconnected);
                        }
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                    MqttEventData::Connected => {
                        if *self.state.borrow() != State::Connected {
                            self.state.send(State::Connected).ok();
                            self.handler.on_event(MqttEvent::Connected);
                        }
                    }
                    MqttEventData::Error(e) => {
                        let is_error = matches!(*self.state.borrow(), State::Error(_));
                        if is_error {
                            self.state.send(State::Error(e.to_string())).ok();
                            self.handler.on_event(MqttEvent::Error(e.to_string()));
                        }
                    }
                    MqttEventData::IncomeMsg(msg) => {
                        self.handler.on_message(msg);
                    }
                }
            }
        });
    }
}
