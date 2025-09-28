#![allow(unused_imports)]
use crate::{Config, MqttEventData};
use anyhow::{Result, anyhow};
use rumqttc::v5::mqttbytes::v5::ConnectReturnCode;
use rumqttc::v5::{AsyncClient, ConnectionError, Event, EventLoop, Incoming, StateError};
use rumqttc::{Connect, Outgoing, TlsConfiguration, Transport};
use std::fmt::Formatter;
use std::io::Read;
use std::time::Duration;
use std::{collections::HashMap, fmt::Debug};
use tokio::{select, time::sleep};
pub(crate) struct Conn {
    pub(crate) eventloop: EventLoop,
}

impl Debug for Conn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn").field("conf", &"conn").finish()
    }
}
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Certificate {
    public_key: String,
    private_key: String,
    certificate: String,
}
fn read(path: &str) -> Vec<u8> {
    let mut file = std::fs::File::open(path).unwrap();
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).unwrap();
    return contents;
}
fn connect(mut opts: Config, tls: bool) -> (AsyncClient, EventLoop) {
    if !tls {
        let ca = read("./AmazonRootCA1.pem");
        let client_cert = read("./device-certificate.pem.crt");
        let client_key = read("./device-private.pem.key");
        let transport = Transport::Tls(TlsConfiguration::Simple {
            ca,
            alpn: None,
            client_auth: Some((client_cert, client_key)),
        });
        opts.set_transport(transport);
    }
    AsyncClient::new(opts, 32)
}

impl Conn {
    pub(crate) fn new(conf: Config) -> (Self, AsyncClient) {
        let (cli, eventloop) = connect(conf.clone(), true);
        (Conn { eventloop }, cli)
    }
    pub(crate) async fn poll_msg(&mut self) -> Option<MqttEventData> {
        let event = self.eventloop.poll().await;

        if let Err(ref e) = event {
            match e {
                ConnectionError::MqttState(s) => match s {
                    StateError::ConnectionAborted | StateError::Io(_) => {
                        return Some(MqttEventData::Disconnected);
                    }
                    _ => {
                        return Some(MqttEventData::Error(format!(
                            "mqtt poll error MqttState:{}",
                            e
                        )));
                    }
                },
                ConnectionError::Io(e) => {
                    log::error!("mqtt poll error Io :{}", e);
                    return Some(MqttEventData::Disconnected);
                }
                ConnectionError::ConnectionRefused(c) => {
                    log::error!("mqtt poll error ConnectionRefused:{:?}", c);
                    match c {
                        ConnectReturnCode::RefusedProtocolVersion => {
                            return Some(MqttEventData::Error(format!("[协议不支持]")));
                        }
                        ConnectReturnCode::BadClientId => {
                            return Some(MqttEventData::Error(format!("[ClientId 不合法]")));
                        }
                        ConnectReturnCode::ServiceUnavailable => {
                            return Some(MqttEventData::Error(format!("[服务器不可用]")));
                        }
                        ConnectReturnCode::BadUserNamePassword => {
                            return Some(MqttEventData::Error(format!("[帐号或密码错误]")));
                        }
                        ConnectReturnCode::NotAuthorized => {
                            return Some(MqttEventData::Error(format!("[授权不通过]")));
                        }
                        _ => {
                            return Some(MqttEventData::Error(format!("[未知错误]")));
                        }
                    }
                }
                _ => return Some(MqttEventData::Error(format!("mqtt poll error:{}", e))),
            }
        }

        let event = event.expect("poll error");
        match event {
            Event::Incoming(msg) => {
                match msg {
                    Incoming::Disconnect(s) => {
                        log::debug!("[incoming]-Disconnect reason:{:?}", s.reason_code);
                        return Some(MqttEventData::Disconnected);
                    }
                    //订阅到数据
                    Incoming::Subscribe(d) => {
                        log::debug!("[incoming]-Subscribe mqtt subscribe:{:?}", d);
                    }

                    Incoming::Publish(d) => {
                        return Some(MqttEventData::IncomeMsg(d));
                    }

                    Incoming::Connect(s, _, _) => {
                        log::debug!("[incoming]-Connnect mqtt connect:{:?}", s);
                    }

                    Incoming::SubAck(s) => {
                        log::debug!("[incoming]-SubAck mqtt sub ack: {:?}", s);
                    }

                    //连接回复
                    Incoming::ConnAck(d) => {
                        log::debug!("[incoming]-ConnAck mqtt conn ack: {:?}", d);
                        match d.code {
                            ConnectReturnCode::Success => {
                                return Some(MqttEventData::Connected);
                            }
                            _ => {
                                return Some(MqttEventData::Error(format!("[{:?}]", d.code)));
                            }
                        }
                    }
                    //服务器回复pong
                    Incoming::PingReq(_) => {
                        log::debug!("[incoming]-pingreq recv mqtt broker pong");
                    }
                    _ => {
                        log::debug!("[incoming]-pingreq msg: {:?}", msg);
                    }
                }
            }
            Event::Outgoing(o) => match o {
                Outgoing::PingReq => {
                    log::trace!("[outgoing] send mqtt broker ping");
                }
                Outgoing::Publish(p) => {
                    log::debug!("[outgoing] publish packId:{}", p);
                }
                Outgoing::Subscribe(p) => {
                    log::debug!("[outgoing] subscribe packId:{}", p);
                }
                Outgoing::Unsubscribe(p) => {
                    log::debug!("[outgoing] unsubscribe packId:{}", p);
                }
                _ => log::debug!("[outgoing] msg:{:?}", o),
            },
        }

        None
    }
}
