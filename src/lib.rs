mod client;
mod conn;
mod manager;
mod router;
pub mod tls;
pub mod types;
use anyhow::anyhow;
pub use client::{Client, MqttClient};
use conn::*;
use manager::*;
pub use router::*;
pub use rumqttc::v5::mqttbytes::QoS;
pub use rumqttc::v5::mqttbytes::v5::{ConnectProperties, Publish as Message};
pub use rumqttc::v5::{AsyncClient, MqttOptions as Config};
pub use tls::*;
pub use types::*;

use std::sync::Arc;
use std::time::Duration;
use tokio::{
    sync::{mpsc, watch},
    time,
};

pub async fn start_with_cfg(
    cfg: Config,
    timeout: Duration,
    producter: mpsc::Sender<MqttMessage>,
) -> MqttResult<MqttClient> {
    //init
    let (conn, c) = Conn::new(cfg);
    let (state_tx, state_rx) = watch::channel(State::Pending);
    let (close_send, close_recv) = watch::channel(false);

    let client = Arc::new(Client::new(state_rx, c, close_send));
    Manager::new(state_tx, conn, producter).run(close_recv.clone());

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

    Client::run(client.clone(), close_recv.clone());
    Ok(client)
}
