use crate::QoS;
use crate::State;
use anyhow::{Result, anyhow};
use bytes::Bytes;
use rumqttc::v5::AsyncClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, watch};

//mqtt client
pub type MqttClient = Arc<Client>;

#[derive(Debug)]
struct Topics(HashMap<String, QoS>);
impl Topics {
    pub fn new() -> Self {
        Topics(HashMap::new())
    }
    pub fn add<T: AsRef<str> + Sync + Send>(&mut self, topic: T, qos: QoS) -> Result<()> {
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
    topics: Mutex<Topics>,
}

impl Client {
    pub fn new(state: watch::Receiver<State>, mqtt: AsyncClient) -> Self {
        let topics = Mutex::new(Topics::new());
        Client {
            state,
            mqtt,
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

    //订阅主题
    pub async fn subscribe(&self, topic: &str, qos: crate::QoS) -> Result<()> {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        self.mqtt.subscribe(topic, qos).await?;
        let mut topics = self.topics.try_lock()?;
        topics.add(topic.to_string(), qos)?;
        Ok(())
    }

    //取消订阅主题
    pub async fn unsubscribe(&self, topic: &str) -> Result<()> {
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

    //重新订阅
    async fn re_subscribe_topic(&self) {
        let topics = self.get_topics();
        log::info!("re subscribe topics len:{}", topics.len());
        for (topic, qos) in topics.iter() {
            if let Err(e) = self.subscribe(topic, *qos).await {
                log::error!("Failed to re subscribe to topic: {}", e);
            }
        }
    }

    pub(crate) fn run(cli: MqttClient) {
        let mut state = cli.state.clone();
        tokio::spawn(async move {
            while state.changed().await.is_ok() {
                if *state.borrow() == State::Connected {
                    cli.re_subscribe_topic().await;
                }
            }
            log::error!("check reconnect subscribe stop");
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

    //发布消息
    pub async fn publish<P, S>(
        &self,
        topic: S,
        payload: P,
        qos: crate::QoS,
        retain: bool,
    ) -> Result<()>
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

    //断开连接
    pub async fn close(&self) -> Result<()> {
        if *self.state.borrow() != State::Connected {
            return Ok(());
        }
        self.mqtt.disconnect().await?;
        Ok(())
    }
}
