use crate::{Conn, Message, MqttMessage, State};

use std::time::Duration;
use tokio::{
    select,
    sync::{mpsc::Sender, watch},
};

pub(crate) struct Manager {
    state: watch::Sender<State>,
    producter: Sender<MqttMessage>,
    conn: Conn,
}

pub(crate) enum MqttEventData {
    Error(String),
    Connected,
    Disconnected,
    IncomeMsg(Message),
}

impl Manager {
    pub(crate) fn new(
        state: watch::Sender<State>,
        conn: Conn,
        producter: Sender<MqttMessage>,
    ) -> Self {
        Manager {
            state,
            producter,
            conn,
        }
    }
    pub(crate) fn run(mut self, mut cancel_recv: watch::Receiver<bool>) {
        tokio::spawn(async move {
            loop {
                select! {
                    _ = cancel_recv.changed() => {
                        if *self.state.borrow() != State::Closed {
                             self.state.send(State::Closed).ok();
                             self.producter.send(MqttMessage::EvtClosed).await.ok();
                        }
                        break;
                    }
                    s=self.conn.poll_msg()=>{
                        let Some(s) = s else {
                            continue;
                        };
                        match s {
                            MqttEventData::Disconnected => {
                                if *self.state.borrow() != State::Disconnected {
                                    self.state.send(State::Disconnected).ok();
                                    self.producter.send(MqttMessage::EvtDisconnected).await.ok();
                                }
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                            MqttEventData::Connected => {
                                if *self.state.borrow() != State::Connected {
                                    self.state.send(State::Connected).ok();
                                    self.producter.send(MqttMessage::EvtConnected).await.ok();
                                }
                            }
                            MqttEventData::Error(e) => {
                                let is_error = matches!(*self.state.borrow(), State::Error(_));
                                if is_error {
                                    self.state.send(State::Error(e.to_string())).ok();
                                    self.producter.send(MqttMessage::EvtError(e.to_string())).await.ok();
                                }
                            }
                            MqttEventData::IncomeMsg(msg) => {
                                self.producter.send(MqttMessage::Msg(msg)).await.ok();
                            }
                        }
                    }
                }
            }
            log::info!("mqtt manager close...");
        });
    }
}
