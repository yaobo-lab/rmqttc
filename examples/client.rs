#![allow(unused_imports)]
use rmqttc::{Config, InitTopics, MqttRouter, Params, Payload, QoS, StateHandle};
use serde::Deserialize;
use serde_json::json;
use std::process;
use std::time::Duration;
use tokio::{signal, time::sleep};
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
async fn mqtt_msg(
    Payload(playload): Payload<String>,
    Params(IdInstAndUnits {
        id,
        instance,
        units,
    }): Params<IdInstAndUnits>,
    StateHandle(()): StateHandle<()>,
) -> anyhow::Result<()> {
    log::info!(
        "id:{},instance:{},units:{} playload:{}",
        id,
        instance,
        units,
        playload
    );
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
    let mut opts = Config::new("client-id-rust-0001", "127.0.0.1", 1883);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_start(false);
    opts.set_credentials("mqtt_usr_name", "12345678");

    let on_event = Box::new(move |evt| {
        log::info!("on_event backback :{:?}", evt);
    });

    //new
    let cli = match rmqttc::start_with_cfg(opts, on_event, Duration::from_secs(10)).await {
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

    // 创建路由
    let mut router = MqttRouter::<()>::new(cli.clone());
    router
        .route("/test/topic/1", mqtt_msg, QoS::AtLeastOnce)
        .await
        .expect("route error");

    router
        .route("/test/topic/2", mqtt_msg, QoS::AtLeastOnce)
        .await
        .expect("route error");

    router
        .route("/test/topic/3", mqtt_msg, QoS::AtLeastOnce)
        .await
        .expect("route error");

    router
        .route("/test/topic/4", mqtt_msg, QoS::AtLeastOnce)
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

    //shutdown
    if let Err(e) = signal::ctrl_c().await {
        log::error!("Failed to listen for the ctrl-c signal: {:?}", e);
    }
    log::info!("ctrl-c signal received done..");
}
