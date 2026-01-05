use crate::{Conn, MqttEvent, MqttEventData, MqttRouter, OnEventCallback, State};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
pub struct Manager {
    pub(crate) router: Arc<MqttRouter>,
    pub(crate) on_event: OnEventCallback,
    pub state: watch::Sender<State>,
}

impl Manager {
    pub(crate) fn new(
        state: watch::Sender<State>,
        router: Arc<MqttRouter>,
        on_event: OnEventCallback,
    ) -> Self {
        Manager {
            state,
            router,
            on_event,
        }
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
                        (self.on_event)(MqttEvent::Disconnected);
                        self.state.send(State::Disconnected).ok();
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                MqttEventData::Connected => {
                    if *self.state.borrow() != State::Connected {
                        (self.on_event)(MqttEvent::Connected);
                        self.state.send(State::Connected).ok();
                    }
                }
                MqttEventData::Error(e) => {
                    let is_error = matches!(*self.state.borrow(), State::Error(_));
                    if is_error {
                        self.state.send(State::Error(e.to_string())).ok();
                        (self.on_event)(MqttEvent::Error(e.to_string()));
                    }
                }
                MqttEventData::IncomeMsg(msg) => {
                    let _ = self.router.clone().dispatch(msg, ()).await;
                }
            }
        }
    }
}
