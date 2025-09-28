## Install

```bash
cargo add rmqttc
```

## Usage

```rust
use rmqttc::{Config, InitTopics, MqttPubCmd, QoS};
use serde_json::json;
use std::process;
use std::time::Duration;
use tokio::{signal, time::sleep};
use toolkit_rs::{
    logger::{self, LogConfig},
    painc::{PaincConf, set_panic_handler},
};
#[tokio::main]
async fn main() {
    set_panic_handler(PaincConf::default());
    logger::setup(LogConfig::default()).unwrap_or_else(|e| {
        println!("log setup err:{}", e);
        process::exit(1);
    });

    //topic
    let mut topics = InitTopics::new();
    topics.add("/test/topic/1", QoS::AtMostOnce).expect("");
    topics.add("/test/topic/2", QoS::AtMostOnce).expect("");

    //config
    let mut opts = Config::new("client-id-rust-102", "10.0.3.188", 1883);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_start(false);
    opts.set_credentials("rust-usr-102", "rust-pwd-102");

    //callback
    let on_msg = Box::new(move |msg| {
        log::info!("on_msg callback:{:?}", msg);
    });

    let on_event = Box::new(move |evt| {
        log::info!("on_event backback :{:?}", evt);
    });

    //new
    let cli = rmqttc::start_with_cfg(opts, on_msg, on_event, topics, Duration::from_secs(10))
        .await
        .expect("start error");
    log::info!("connect success");

    let msg = MqttPubCmd {
        topic: "/hello/yaobo".to_string(),
        qos: QoS::AtMostOnce,
        retain: false,
        last_will: None,
        data: json!("hello rust 1"),
    };
    cli.publish(msg).await.expect("publish error");

    let msg = MqttPubCmd {
        topic: "/hello/yaobo".to_string(),
        qos: QoS::AtMostOnce,
        retain: false,
        last_will: None,
        data: json!("hello rust 2"),
    };
    cli.publish(msg).await.expect("publish error");

    sleep(Duration::from_secs(3)).await;

    cli.subscribe("/test/topic/3", QoS::AtMostOnce)
        .await
        .expect("subscribe error");

    cli.subscribe("/test/topic/4", QoS::AtMostOnce)
        .await
        .expect("subscribe error");

    cli.subscribe("/test/1002/#", QoS::AtMostOnce)
        .await
        .expect("subscribe error");

    cli.subscribe("/test/2002/+/hello", QoS::AtMostOnce)
        .await
        .expect("subscribe error");

    //cli.disconnect().await.expect("disconnect error");

    log::info!("wait signal shutdonw..");

    //shutdown
    if let Err(e) = signal::ctrl_c().await {
        log::error!("Failed to listen for the ctrl-c signal: {:?}", e);
    }
    log::info!("ctrl-c signal received done..");
}

```
