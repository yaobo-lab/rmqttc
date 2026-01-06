## Install

```bash
cargo add rmqttc
```

## Usage

```rust
#![allow(unused_imports, dead_code)]
use rmqttc::{Config, IHandler, Message, MqttEvent, MqttRouter, Params, Payload, QoS, StateHandle};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use std::{process, sync::Arc};
use tokio::{signal, sync::mpsc, time::sleep};
use toolkit_rs::{
    logger::{self, LogConfig},
    painc::{PaincConf, set_panic_handler},
};

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
    StateHandle(_): StateHandle<()>,
) -> anyhow::Result<()> {
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
    StateHandle(()): StateHandle<()>,
) -> anyhow::Result<()> {
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
    StateHandle(()): StateHandle<()>,
) -> anyhow::Result<()> {
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
    let mut opts = Config::new("client-id-0001", "127.0.0.1", 1883);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_start(false);
    opts.set_credentials("mqtt_usr_name", "12345678");

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

    cli.publish(
        "/hello/yaobo",
        "playload: hello world 1",
        QoS::AtLeastOnce,
        false,
    )
    .await
    .expect("publish error");

    //创建路由
    let mut router = MqttRouter::<()>::new(cli.clone());
    router
        .route("hello/rumqtt", mqtt_msg, QoS::AtLeastOnce)
        .await
        .expect("route error");

    //test/+/set-temperature/+/+
    router
        .route(
            "test/{id}/set-temperature/{instance}/{units}",
            mqtt_msg2,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    router
        .route("/test/topic/3", mqtt_msg3, QoS::AtLeastOnce)
        .await
        .expect("route error");

    cli.publish(
        "/hello/yaobo",
        "playload: hello world 2",
        QoS::AtLeastOnce,
        false,
    )
    .await
    .expect("publish error");

    tokio::spawn(async move {
        loop {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = router.dispatch(msg, ()).await {
                    log::error!("dispatch error: {}", e);
                }
            }
        }
    });

    if let Err(e) = signal::ctrl_c().await {
        log::error!("Failed to listen for the ctrl-c signal: {:?}", e);
    }
    log::info!("ctrl-c signal received done..");
}

```
