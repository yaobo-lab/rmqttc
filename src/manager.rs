use crate::{Conn, MqttEvent, MqttEventData, State, handler::IHandler};
use std::time::Duration;
use tokio::sync::watch;
pub struct Manager {
    pub state: watch::Sender<State>,
    handler: Box<dyn IHandler>,
}
impl Manager {
    pub(crate) fn new(state: watch::Sender<State>, handler: Box<dyn IHandler>) -> Self {
        Manager { state, handler }
    }

    //运行
    pub(crate) async fn run(self, mut conn: Conn) {
        loop {
            let s = conn.poll_msg().await;
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
    }
}
