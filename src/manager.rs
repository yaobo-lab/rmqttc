use crate::{Conn, MqttEvent, MqttEventData, OnEventCallback, State};
use std::time::Duration;
use tokio::sync::watch;
pub struct Manager {
    pub state: watch::Sender<State>,
}

impl Manager {
    pub(crate) fn new(state: watch::Sender<State>) -> Self {
        Manager { state }
    }
    //运行
    pub(crate) async fn run(self, mut conn: Conn, on_event: OnEventCallback) {
        loop {
            let s = conn.poll_msg().await;
            let Some(s) = s else {
                continue;
            };
            match s {
                MqttEventData::Disconnected => {
                    if *self.state.borrow() != State::Disconnected {
                        (on_event)(MqttEvent::Disconnected);
                        self.state.send(State::Disconnected).ok();
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                MqttEventData::Connected => {
                    if *self.state.borrow() != State::Connected {
                        (on_event)(MqttEvent::Connected);
                        self.state.send(State::Connected).ok();
                    }
                }
                MqttEventData::Error(e) => {
                    let is_error = matches!(*self.state.borrow(), State::Error(_));
                    if is_error {
                        self.state.send(State::Error(e.to_string())).ok();
                        (on_event)(MqttEvent::Error(e.to_string()));
                    }
                }
                MqttEventData::IncomeMsg(_msg) => {
                    //  let _ = router.clone().dispatch(msg, ()).await;
                }
            }
        }
    }
}
