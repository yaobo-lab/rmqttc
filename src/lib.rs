#![allow(dead_code)]
mod client;
mod conn;
mod manager;
use anyhow::{Result, anyhow};
use bytes::Bytes;
pub use client::*;
pub(crate) use conn::*;
pub use manager::*;
pub use rumqttc::v5::mqttbytes::QoS;
pub use rumqttc::v5::mqttbytes::v5::{ConnectProperties, Publish as IncomeMessage};
pub use rumqttc::v5::{AsyncClient, MqttOptions as Config};
use serde::Serializer;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time;
//初始化路由
pub struct InitTopics(HashMap<String, QoS>);

impl InitTopics {
    pub fn new() -> Self {
        InitTopics(HashMap::new())
    }
    pub fn add<T: AsRef<str> + Sync + Send>(&mut self, topic: T, qos: QoS) -> Result<()> {
        let topic_ref = topic.as_ref();
        if !self.0.contains_key(topic_ref) {
            self.0.insert(topic_ref.to_string(), qos);
        }
        Ok(())
    }
    pub fn get_topics(&self) -> HashMap<String, QoS> {
        self.0.clone()
    }
    pub fn remove_topic<T: AsRef<str>>(&mut self, topic: T) {
        let topic_ref = topic.as_ref();
        self.0.remove(topic_ref);
    }
}

//事件回调
pub type OnEventCallback = Box<dyn Fn(MqttEvent) + Send + Sync + 'static>;
//消息回调
pub type OnMessageCallback = Box<dyn Fn(IncomeMessage) + Send + Sync + 'static>;

//MQTT 状态
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum State {
    Pending,
    Connected,
    Disconnected,
    Closed,
    Error(String),
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Pending => write!(f, "pending"),
            State::Connected => write!(f, "connected"),
            State::Disconnected => write!(f, "disconnected"),
            State::Closed => write!(f, "closed"),
            State::Error(s) => write!(f, "Error: {}", s),
        }
    }
}

//MQTT事件
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum MqttEvent {
    Connected,
    Disconnected,
    Closed,
}
impl MqttEvent {
    pub fn to_string(&self) -> String {
        match self {
            MqttEvent::Connected => format!("connected"),
            MqttEvent::Disconnected => format!("disconnected"),
            MqttEvent::Closed => format!("closed"),
        }
    }
}

enum MqttEventData {
    Error(String),
    Connected,
    Disconnected,
    IncomeMsg(IncomeMessage),
}

fn qos_to_u8(qos: &QoS) -> u8 {
    match qos {
        QoS::AtMostOnce => 0,
        QoS::AtLeastOnce => 1,
        QoS::ExactlyOnce => 2,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MqttPubCmd {
    pub topic: String,
    #[serde(serialize_with = "serialize_qos", deserialize_with = "deserialize_qos")]
    pub qos: QoS,
    pub retain: bool,
    pub last_will: Option<bool>,
    pub data: Value,
}

impl Default for MqttPubCmd {
    fn default() -> Self {
        MqttPubCmd {
            topic: "".to_string(),
            qos: QoS::AtMostOnce,
            retain: false,
            last_will: None,
            data: Value::Null,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MqttSubCmd {
    pub topic: String,
    #[serde(serialize_with = "serialize_qos", deserialize_with = "deserialize_qos")]
    pub qos: QoS,
}

fn serialize_qos<S>(qos: &QoS, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let num = qos_to_u8(qos);
    serializer.serialize_u8(num)
}

fn deserialize_qos<'de, D>(deserializer: D) -> Result<QoS, D::Error>
where
    D: Deserializer<'de>,
{
    let v = u8::deserialize(deserializer)?;
    let q = rumqttc::v5::mqttbytes::qos(v).unwrap_or(QoS::AtMostOnce);
    Ok(q)
}

#[derive(Debug, PartialEq)]
pub enum MqttMsg {
    Pub(MqttPubCmd),
    Sub(MqttSubCmd),
    UnSub(String),
    Closed,
}

pub async fn start_with_cfg(
    cfg: Config,
    on_msg: OnMessageCallback,
    on_event: OnEventCallback,
    topics: InitTopics,
    timeout: Duration,
) -> Result<Client> {
    //init
    let (state_tx, state_rx) = watch::channel(State::Pending);
    let (cmd_sender, cmd_receiver) = mpsc::channel::<MqttMsg>(128);
    let client = Client::new(state_rx, cmd_sender);

    //mqtt
    let topics = topics.get_topics();
    let mut man = Manager::new(cfg, state_tx, on_msg, on_event, topics);
    tokio::spawn(async move { man.run(cmd_receiver).await });

    let mut timeout = timeout.as_secs();
    if timeout <= 0 {
        timeout = 10;
    }
    let mut re_count = 0;
    while !client.connected() {
        if re_count > timeout {
            log::error!("connect timeout {} s", timeout);
            client.close().await?;
            time::sleep(Duration::from_millis(100)).await;
            return Err(anyhow!("连接超时，请检查网络是否正常.."));
        }
        if let Some(s) = client.state_is_error() {
            return Err(anyhow!("连接失败: {}", s));
        }
        log::debug!("wait connect..");
        time::sleep(Duration::from_secs(1)).await;
        re_count += 1;
    }

    Ok(client)
}

pub fn to_topic(topic: &str, skuid: &str, uuid: &str) -> String {
    topic.replace("{skuid}", skuid).replace("{uuid}", uuid)
}

// /aam/sub/request/2928/10002 是否符合 /aam/sub/request/+/+
pub fn topic_match_one(topic: &str, topic_filter: &str) -> bool {
    if topic.to_lowercase() == topic_filter.to_lowercase() {
        return true;
    }

    let topic_parts: Vec<&str> = topic.split('/').collect();
    let pattern_parts: Vec<&str> = topic_filter.split('/').collect();
    if topic_parts.len() != pattern_parts.len() {
        return false;
    }

    for (i, pattern_part) in pattern_parts.iter().enumerate() {
        if *pattern_part != "+" && *pattern_part != topic_parts[i] {
            return false;
        }
    }
    true
}

// /aam/sub/request/2928/10002 是否符合 /aam/sub/request/+/+
// 提取 + + 里的值
pub fn topic_get_match_one(topic: &str, topic_filter: &str) -> Option<Vec<String>> {
    let topic_parts: Vec<&str> = topic.split('/').collect();
    let pattern_parts: Vec<&str> = topic_filter.split('/').collect();
    if topic_parts.len() != pattern_parts.len() {
        return None;
    }

    let mut values = Vec::new();
    for (i, pattern_part) in pattern_parts.iter().enumerate() {
        if *pattern_part == "+" {
            values.push(topic_parts[i].to_string());
        } else if *pattern_part != topic_parts[i] {
            return None; // 静态部分不匹配
        }
    }
    Some(values)
}

//test/topic/1/21/2232  能配符配置 test/topic/#
pub fn topic_match_all(topic: &str, filter: &str) -> bool {
    if topic.to_lowercase() == filter.to_lowercase() {
        return true;
    }

    let topic_parts: Vec<&str> = topic.split('/').collect();
    let filter_parts: Vec<&str> = filter.split('/').collect();

    // 检查是否存在 # 通配符
    if let Some(pos) = filter_parts.iter().position(|&x| x == "#") {
        // 检查静态前缀是否匹配
        if topic_parts.len() < pos {
            return false;
        }

        for i in 0..pos {
            if filter_parts[i] != "+" && filter_parts[i] != topic_parts[i] {
                return false;
            }
        }

        true
    } else {
        // 没有 # 通配符时需要完全匹配
        if topic_parts.len() != filter_parts.len() {
            return false;
        }

        for (i, &filter_part) in filter_parts.iter().enumerate() {
            if filter_part != "+" && filter_part != topic_parts[i] {
                return false;
            }
        }

        true
    }
}

// bytes to string
pub fn bytes_to_string(b: &Bytes) -> Option<String> {
    std::str::from_utf8(b).ok().map(|s| s.to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;
    #[test]
    fn test_topic_is_match() {
        let topic = "test/topic/1/21";
        let topic_filter = "test/topic/+/+";
        let res = topic_match_one(topic, topic_filter);
        println!("res:===> {}", res);
    }

    #[test]
    fn test_topic_get_match() {
        let topic = "test/topic/1/21";
        let topic_filter = "test/topic/+/+";
        let res = topic_get_match_one(topic, topic_filter);
        println!("res:===> {:?}", res);
    }

    #[test]
    fn test_topic_match_all() {
        let topic = "test/topic/1/21/2232";
        let topic_filter = "test/topic/11/#";
        let res = topic_match_all(topic, topic_filter);
        println!("res:===> {}", res);
    }

    #[test]
    fn test_serialize() {
        let d = MqttPubCmd {
            topic: "test".to_string(),
            qos: QoS::AtMostOnce,
            retain: false,
            last_will: None,
            data: json!("test"),
        };

        let s = serde_json::to_string(&d).unwrap();
        println!("{}", s)
    }

    #[test]
    fn test_deserialize() {
        let json_str = r#"{"topic":"test","qos":1,"retain":false,"last_will":null,"data":"test"}"#;
        let s: MqttPubCmd = serde_json::from_str(json_str).unwrap();
        println!("{:?}", s)
    }
}
