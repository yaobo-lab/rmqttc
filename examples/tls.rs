use rumqttc::v5::{AsyncClient, MqttOptions};
use rumqttc::{TlsConfiguration, Transport};
use std::sync::Arc;
use std::time::Duration;
use toolkit_rs::{
    logger::{self, LogConfig},
    painc::{PaincConf, set_panic_handler},
};
#[tokio::main(flavor = "current_thread")]
async fn main() {
    set_panic_handler(PaincConf::default());
    let lcfg = LogConfig {
        style: logger::LogStyle::Line,
        // level: 5,
        filters: Some(vec!["rumqttc".to_string()]),
        ..LogConfig::default()
    };
    logger::setup(lcfg).unwrap_or_else(|e| {
        println!("log setup err:{}", e);
        std::process::exit(1);
    });

    let uri = "ssl://10.0.9.2:8883?client_id=dev-11271097";
    let mut opts = MqttOptions::parse_url(uri).unwrap();
    opts.set_clean_start(false)
        .set_keep_alive(Duration::from_secs(30))
        .set_credentials("uname", "passwd1234567")
        .set_session_expiry_interval(Some(600));

    //
    let cfg = rmqttc::TlsCert {
        device_certs: "./etc/device.crt".to_string(),
        device_private_key: "./etc/device_private.key".to_string(),
        ca_certs: "./etc/device_ca.pem".to_string(),
    };
    let mut config = rmqttc::default_tls_client_config(cfg).unwrap();

    config
        .dangerous()
        .set_certificate_verifier(Arc::new(rmqttc::NoOpVerifier));

    let transport = Transport::Tls(TlsConfiguration::Rustls(Arc::new(config)));
    opts.set_transport(transport);

    println!("开始连接...");
    let (_client, mut eventloop) = AsyncClient::new(opts, 10);

    loop {
        match eventloop.poll().await {
            Ok(v) => println!("Event = {v:?}"),
            Err(e) => {
                println!("Error = {e:?}");
                break;
            }
        }
    }
}
