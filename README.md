# rmqttc

一个基于 Rust 的高性能 MQTTV5.0 客户端库，提供异步、类型安全的 MQTT 消息处理能力。

## 功能特性

- ✨ **异步 I/O** - 基于 Tokio 的高效异步运行时
- 🎯 **类型安全** - 强类型参数提取和消息路由
- 🛣️ **智能路由** - 支持主题通配符和自定义路由处理
- 🔒 **安全连接** - 支持 TLS/SSL 加密传输
- 📊 **灵活配置** - 丰富的连接配置选项
- 🎭 **事件系统** - 完整的事件和错误处理机制

## 安装

```bash
cargo add rmqttc
```

## 快速开始

### 基础连接

```rust
use rmqttc::{Config, MqttClient};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // 创建配置
    let config = Config::new("client_id", "127.0.0.1", 1883);
    
    // 连接到服务器
    let (client, mut rx) = rmqttc::start_with_cfg(config, Duration::from_secs(10), tx).await
        .expect("Failed to connect");
    
    println!("Connected successfully!");
}
```

### 订阅主题和处理消息

```rust
#![allow(dead_code)]

use rmqttc::{
    Config, ConnectProperties, MqttClient, MqttResult, MqttRouter, Params, Payload, QoS,
    StateHandle, types,
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

async fn mqtt_event_msg(
    Payload(evt): Payload<String>,
    Params(_): Params<serde_json::Value>,
    StateHandle(_): StateHandle<InstanceHandle>,
) -> MqttResult {
    log::info!("mqtt event msg-----> {}", evt);
    Ok(())
}
async fn mqtt_event_err_msg(
    Payload(evt): Payload<String>,
    Params(_): Params<serde_json::Value>,
    StateHandle(_): StateHandle<InstanceHandle>,
) -> MqttResult {
    log::info!("mqtt err msg----->  {}", evt);
    Ok(())
}
async fn mqtt_unkonw_msg(
    Payload(evt): Payload<String>,
    Params(_): Params<serde_json::Value>,
    StateHandle(_): StateHandle<InstanceHandle>,
) -> MqttResult {
    log::info!("mqtt unkonw msg-----> {}", evt);
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
    let mut opts = Config::new("01bd0719e0c3f492c261cf102", "127.0.0.1", 1883);
    let properties = ConnectProperties {
        user_properties: vec![ 
            ("Version".to_string(), "1.0.0".into()), 
        ],
        ..ConnectProperties::default()
    };
    opts.set_connect_properties(properties);
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_start(false);
    opts.set_credentials(
        "hello-0001",
        "oekdwNaYKhd+g==",
    );

    let (tx, mut rx) = mpsc::channel(64);
    let cli = match rmqttc::start_with_cfg(opts, Duration::from_secs(10), tx).await {
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
            "/sub/bb/00ab1bd0719e0c3f4b0ec92c261cf102",
            mqtt_msg,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    //test/+/set-temperature/+/+
    router
        .subscribe(
            "/sub/aa/00ab1bd0719e0c3f4b0ec92c261cf102",
            mqtt_msg2,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    router
        .subscribe(
            "/sub/msg/AAM9006/0050C070895A",
            mqtt_msg2,
            QoS::AtLeastOnce,
        )
        .await
        .expect("route error");

    router
        .add(types::EvtTopic, mqtt_event_msg)
        .expect("route error");

    router
        .add(types::UnkonwTopic, mqtt_unkonw_msg)
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

```

## 核心概念

### 1. **配置（Config）**
创建 MQTT 连接的配置对象，包含服务器地址、端口和客户端ID。

```rust
let mut config = Config::new("client_id", "broker.example.com", 1883);
config.set_keep_alive(Duration::from_secs(30));
config.set_clean_start(false);
config.set_credentials("username", "password");
```

### 2. **消息处理器（Handler）**
异步处理函数，接收消息负载、路由参数和应用状态。

```rust
async fn handle_message(
    Payload(msg): Payload<String>,           // 消息内容
    Params(params): Params<MyParams>,         // 路由参数
    StateHandle(state): StateHandle<AppState>, // 应用状态
) -> MqttResult {
    // 处理消息
    Ok(())
}
```

### 3. **消息路由（MqttRouter）**
根据主题订阅消息，支持通配符和参数提取。

```rust
let mut router = MqttRouter::new(client);

// 订阅特定主题
router.subscribe(
    "/device/+/temperature",
    handle_temperature,
    QoS::AtLeastOnce
).await?;

// 添加事件处理器
router.add(types::EvtTopic, handle_event)?;
```

### 4. **消息派发**
在事件循环中接收并派发消息到对应的处理器。

```rust
tokio::spawn(async move {
    while let Some(msg) = rx.recv().await {
        if let Err(e) = router.dispatch(msg, state.clone()).await {
            eprintln!("Error dispatching message: {}", e);
        }
    }
});
```

## 主题通配符支持

- `+` - 匹配单级（例如 `/device/+/temp` 匹配 `/device/001/temp`）
- `#` - 匹配多级（例如 `/device/#` 匹配 `/device/001/temp`）

## 连接选项

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `set_keep_alive()` | 心跳间隔 | 60 秒 |
| `set_clean_start()` | 清空会话 | true |
| `set_credentials()` | 用户名和密码 | 无 |
| `set_connect_properties()` | 连接属性 | 空 |

## 错误处理

所有异步操作返回 `MqttResult` 类型，处理连接和消息错误：

```rust
match client.publish("topic", "message", QoS::AtLeastOnce).await {
    Ok(_) => println!("Published"),
    Err(e) => eprintln!("Publish error: {}", e),
}
```

## 许可

请参阅 LICENSE 文件。
