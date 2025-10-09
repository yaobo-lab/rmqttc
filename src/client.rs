use crate::{MqttMsg, MqttPubCmd, MqttSubCmd, State};
use anyhow::{Result, anyhow};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone)]
pub struct Client {
    pub state: watch::Receiver<State>,
    cmd_sender: mpsc::Sender<MqttMsg>,
}

impl Client {
    pub fn new(state: watch::Receiver<State>, cmd_sender: mpsc::Sender<MqttMsg>) -> Self {
        Client { state, cmd_sender }
    }
    pub fn empty() -> Self {
        Client {
            state: watch::channel(State::Closed).1,
            cmd_sender: mpsc::channel::<MqttMsg>(1).0,
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
        let cmd = MqttMsg::Sub(MqttSubCmd {
            topic: topic.to_string(),
            qos,
        });
        self.cmd_sender.send(cmd).await?;
        Ok(())
    }

    //取消订阅主题
    pub async fn unsubscribe(&self, topic: &str) -> Result<()> {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        let cmd = MqttMsg::UnSub(topic.to_string());
        self.cmd_sender.send(cmd).await?;
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
    pub async fn publish(&self, msg: MqttPubCmd) -> Result<()> {
        if *self.state.borrow() != State::Connected {
            return Err(anyhow!("mqtt not connected"));
        }
        let cmd = MqttMsg::Pub(msg);
        self.cmd_sender.send(cmd).await?;
        Ok(())
    }

    //断开连接
    pub async fn close(&self) -> Result<()> {
        if self.cmd_sender.is_closed() {
            return Ok(());
        }
        self.cmd_sender.send(MqttMsg::Closed).await?;
        Ok(())
    }
}
