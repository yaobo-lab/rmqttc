#![allow(dead_code)]

use rmqttc::{
    Config, ConnectProperties, IHandler, Message, MqttClient, MqttEvent, MqttResult, MqttRouter,
    Params, Payload, QoS, StateHandle,
};
use serde::Deserialize;
use std::time::Duration;
use std::{process, sync::Arc};

use tokio::{
    signal,
    sync::{RwLock, mpsc},
};
use toolkit_rs::{
    logger::{self, LogConfig},
    painc::{PaincConf, set_panic_handler},
};

#[derive(Default)]
pub struct Instance {
    mqtt: RwLock<Option<MqttClient>>,
    tmp_prefix: RwLock<String>,
}
pub type InstanceHandle = Arc<Instance>;
impl Instance {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_mqtt(&self, client: MqttClient) {
        *self.mqtt.write().await = Some(client);
    }
    pub async fn get_mqtt(&self) -> Option<MqttClient> {
        self.mqtt.read().await.clone()
    }

    pub async fn set_tmp_prefix(&self, prefix: String) {
        *self.tmp_prefix.write().await = prefix;
    }
    pub async fn get_tmp_prefix(&self) -> String {
        self.tmp_prefix.read().await.clone()
    }
}

#[derive(Deserialize)]
pub struct IdInstAndUnits {
    id: String,
    instance: String,
    units: String,
}

struct MyHandler {
    tx: mpsc::Sender<Message>,
}

impl MyHandler {
    fn new(tx: mpsc::Sender<Message>) -> Self {
        MyHandler { tx }
    }
}
impl IHandler for MyHandler {
    fn on_message(&self, msg: Message) {
        match self.tx.try_send(msg) {
            Ok(()) => {}
            Err(e) => log::error!("{}", e),
        }
    }
    fn on_event(&self, event: MqttEvent) {
        log::info!("event = {}", event.to_string());
    }
}

async fn mqtt_msg(
    Payload(playload): Payload<String>,
    Params(_): Params<serde_json::Value>,
    StateHandle(_): StateHandle<InstanceHandle>,
) -> MqttResult {
    log::info!("1. playload:{}", playload);
    Ok(())
}

async fn mqtt_msg2(
    Payload(playload): Payload<String>,
    Params(IdInstAndUnits {
        id,
        instance,
        units,
    }): Params<IdInstAndUnits>,
    StateHandle(_): StateHandle<InstanceHandle>,
) -> MqttResult {
    log::info!(
        "2. \n id:{},instance:{},units:{} \n playload:{}",
        id,
        instance,
        units,
        playload
    );

    Ok(())
}

async fn mqtt_msg3(
    Payload(playload): Payload<String>,
    Params(_): Params<serde_json::Value>,
    StateHandle(_): StateHandle<InstanceHandle>,
) -> MqttResult {
    log::info!("3.playload:{}", playload);
    Ok(())
}

#[tokio::main]
async fn main() {
    set_panic_handler(PaincConf::default());
    let lcfg = LogConfig {
        style: logger::LogStyle::Line,
        filters: Some(vec!["rumqttc".to_string()]),
        ..LogConfig::default()
    };
    logger::setup(lcfg).unwrap_or_else(|e| {
        println!("log setup err:{}", e);
        process::exit(1);
    });

    //config
    let mut opts = Config::new("00ab1bd0719e0c3f4b0ec92c261cf102", "10.0.3.36", 1883);
    let properties = ConnectProperties {
        user_properties: vec![
            ("EndpointId".to_string(), "1".to_string()),
            ("Version".to_string(), "1.0.0".into()),
            ("ClassId".to_string(), "AAM".to_string()),
        ],
        ..ConnectProperties::default()
    };
    opts.set_connect_properties(properties);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_start(false);
    opts.set_credentials(
        "aam_sub_panel_ktv",
        "GSBpY84VUuudkKGFdVJR7o1uF6TnxCDn23bgBNiMWNovscDgV1Cgjny8Zq0ADFC04STs8GC0qQj/MwNaYKhd+g==",
    );

    let (tx, mut rx) = mpsc::channel(64);
    let handler = Box::new(MyHandler::new(tx));
    let cli = match rmqttc::start_with_cfg(opts, Duration::from_secs(10), handler).await {
        Ok(cli) => cli,
        Err(e) => {
            log::error!("start error:{}", e);
            process::exit(1);
        }
    };

    log::info!("---------connect success---------");

    //init instance
    let state = Arc::new(Instance::new());
    state.set_mqtt(cli.clone()).await;
    state.set_tmp_prefix("yaobo".into()).await;

    //创建路由
    let mut router = MqttRouter::<InstanceHandle>::new(cli.clone());
    router
        .subscribe(
            "/aam/sub/request/panel_ktv/00ab1bd0719e0c3f4b0ec92c261cf102",
            mqtt_msg,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    //test/+/set-temperature/+/+
    router
        .subscribe(
            "/aam/sub/upgrade/request/panel_ktv/00ab1bd0719e0c3f4b0ec92c261cf102",
            mqtt_msg2,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    router
        .subscribe(
            "/aam/shop/sub/msg/AAM9006/0050C070895A",
            mqtt_msg2,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    tokio::spawn(async move {
        loop {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = router.dispatch(msg, state.clone()).await {
                    log::error!("dispatch error: {}", e);
                }
            }
        }
    });

    log::info!("mqtt state: {}", cli.state());
    log::info!("----------wait for ctrl-c signal----------");
    if let Err(e) = signal::ctrl_c().await {
        log::error!("Failed to listen for the ctrl-c signal: {:?}", e);
    }
    log::info!("ctrl-c signal received done..");
}
