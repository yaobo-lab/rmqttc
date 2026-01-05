use crate::State;
use anyhow::{Result, anyhow};
use bytes::Bytes;
use rumqttc::v5::AsyncClient;
use std::sync::Arc;
use tokio::sync::watch;
//mqtt client
pub type MqttClient = Arc<Client>;

#[derive(Debug, Clone)]
pub struct Client {
    state: watch::Receiver<State>,
    mqtt: AsyncClient,
}

impl Client {
    pub fn new(state: watch::Receiver<State>, mqtt: AsyncClient) -> Self {
        Client { state, mqtt }
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
        Ok(())
    }

    //取消订阅主题
    pub async fn unsubscribe(&self, topic: &str) -> Result<()> {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        self.mqtt.unsubscribe(topic).await?;
        Ok(())
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
