use crate::{MqttResult, PublishMessage, QoS, State};
use anyhow::anyhow;
use bytes::Bytes;
use rumqttc::v5::AsyncClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{
    select,
    sync::{Mutex, watch},
};

pub type MqttClient = Arc<Client>;

#[derive(Debug)]
struct Topics(HashMap<String, QoS>);

impl Topics {
    pub fn new() -> Self {
        Topics(HashMap::new())
    }
    pub fn add<T: AsRef<str> + Sync + Send>(&mut self, topic: T, qos: QoS) -> MqttResult {
        let topic_ref = topic.as_ref();
        if !self.0.contains_key(topic_ref) {
            self.0.insert(topic_ref.to_string(), qos);
        }
        Ok(())
    }
    pub fn get(&self) -> HashMap<String, QoS> {
        self.0.clone()
    }
    pub fn remove<T: AsRef<str>>(&mut self, topic: T) {
        let topic_ref = topic.as_ref();
        self.0.remove(topic_ref);
    }
}
#[derive(Debug)]
pub struct Client {
    state: watch::Receiver<State>,
    mqtt: AsyncClient,
    close: watch::Sender<bool>,
    topics: Mutex<Topics>,
}

impl Client {
    pub fn new(
        state: watch::Receiver<State>,
        mqtt: AsyncClient,
        close: watch::Sender<bool>,
    ) -> Self {
        let topics = Mutex::new(Topics::new());
        Client {
            state,
            mqtt,
            close,
            topics,
        }
    }

    pub fn state(&self) -> String {
        match *self.state.borrow() {
            State::Pending => "pending".to_string(),
            State::Connected => "connected".to_string(),
            State::Disconnected => "disconnected".to_string(),
            State::Closed => "closed".to_string(),
            State::Error(ref s) => format!("error:{}", s),
        }
    }

    pub async fn subscribe(&self, topic: &str, qos: crate::QoS) -> MqttResult {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        self.mqtt.subscribe(topic, qos).await?;
        let mut topics = self.topics.try_lock()?;
        topics.add(topic.to_string(), qos)?;
        Ok(())
    }

    pub async fn unsubscribe(&self, topic: &str) -> MqttResult {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        self.mqtt.unsubscribe(topic).await?;
        let mut topics = self.topics.try_lock()?;
        topics.remove(topic.to_string());
        Ok(())
    }

    fn get_topics(&self) -> HashMap<String, QoS> {
        if let Ok(topics) = self.topics.try_lock() {
            return topics.get();
        }
        HashMap::new()
    }

    async fn re_subscribe_topic(&self) {
        let topics = self.get_topics();
        log::info!("re subscribe topics len:{}", topics.len());
        for (topic, qos) in topics.iter() {
            if let Err(e) = self.subscribe(topic, *qos).await {
                log::error!("Failed to re subscribe to topic: {}", e);
            }
        }
    }

    pub(crate) fn run(cli: MqttClient, mut close_recv: watch::Receiver<bool>) {
        let mut state = cli.state.clone();
        tokio::spawn(async move {
            loop {
                select! {
                     _ = close_recv.changed() => {
                        break;
                    },
                    _ = state.changed() => {
                       log::info!("mqtt state change");
                       if *state.borrow() == State::Connected {
                          cli.re_subscribe_topic().await;
                       }
                    }
                }
            }
            log::info!("mqtt client close...");
        });
    }

    pub fn connected(&self) -> bool {
        *self.state.borrow() == State::Connected
    }

    pub fn state_is_error(&self) -> Option<String> {
        match *self.state.borrow() {
            State::Error(ref s) => Some(s.clone()),
            _ => None,
        }
    }

    pub async fn publish<P, S>(
        &self,
        topic: S,
        payload: P,
        qos: crate::QoS,
        retain: bool,
    ) -> MqttResult
    where
        P: Into<Bytes>,
        S: Into<String>,
    {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        self.mqtt.publish(topic, qos, retain, payload).await?;
        Ok(())
    }

    pub async fn publish_msg(&self, msg: PublishMessage) -> MqttResult {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        self.mqtt
            .publish(
                msg.topic,
                msg.qos,
                msg.retain,
                crate::json_value_into_bytes(msg.data),
            )
            .await?;
        Ok(())
    }

    pub async fn close(&self) -> MqttResult {
        if *self.state.borrow() == State::Closed {
            return Ok(());
        }
        if !self.close.is_closed() {
            self.close.send(true)?;
        }
        self.mqtt.disconnect().await.ok();
        Ok(())
    }
}
