use std::process;
use tokio::{signal, time::sleep};
use toolkit_rs::{
    logger::{self, LogConfig},
    painc::{PaincConf, set_panic_handler},
};

use rmqttc::{AsyncClient, Config, QoS};
use std::time::Duration;
use tokio::{task, time};

#[tokio::main]
async fn main() {
    set_panic_handler(PaincConf::default());
    logger::setup(LogConfig::default()).unwrap_or_else(|e| {
        println!("log setup err:{}", e);
        process::exit(1);
    });

    //mqtt
    let mut opts = Config::new("client-id-rust-100", "10.0.3.188", 1883);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_start(false);
    //Username and password
    opts.set_credentials("rust-usr", "rust-pwd");

    let (client, mut eventloop) = AsyncClient::new(opts, 32);
    client
        .subscribe("hello/rumqtt", QoS::AtMostOnce)
        .await
        .expect(" subscribe failed");

    let client_clone = client.clone();
    task::spawn(async move {
        for _ in 0..10 {
            client_clone
                .publish("/hello/yaobo", QoS::AtMostOnce, false, "fuck you")
                .await
                .unwrap();
            time::sleep(Duration::from_millis(100)).await;
        }
    });

    task::spawn(async move {
        log::info!("wait to disconnect...");
        sleep(Duration::from_secs(15)).await;
        client.disconnect().await.expect(" disconnect failed ");
        log::info!("disconnect success");
    });

    while let Ok(msg) = eventloop.poll().await {
        log::info!("Received = {:?}", msg);
    }

    log::info!("eventloop exit");

    //shutdown
    if let Err(e) = signal::ctrl_c().await {
        log::error!("Failed to listen for the ctrl-c signal: {:?}", e);
    }
    log::info!("ctrl-c signal received done..");
}
