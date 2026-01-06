#![allow(unused_imports)]
use crate::{Config, MqttEventData};
use anyhow::{Result, anyhow};
use rumqttc::v5::mqttbytes::v5::{ConnectReturnCode, PubAckReason, SubscribeReasonCode};
use rumqttc::v5::{AsyncClient, ConnectionError, Event, EventLoop, Incoming, StateError};
use rumqttc::{Connect, Outgoing, TlsConfiguration, Transport};
use std::fmt::Formatter;
use std::io::Read;
use std::time::Duration;
use std::{collections::HashMap, fmt::Debug};
use tokio::{select, time::sleep};

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

pub fn cfg_tls_transport(
    mut opts: Config,
    ca_path: &str,
    client_cert: &str,
    client_key: &str,
) -> Config {
    //"./AmazonRootCA1.pem"
    let ca = read(ca_path);
    //./device-certificate.pem.crt
    let client_cert = read(client_cert);
    //"./device-private.pem.key"
    let client_key = read(client_key);
    let transport = Transport::Tls(TlsConfiguration::Simple {
        ca,
        alpn: None,
        client_auth: Some((client_cert, client_key)),
    });
    opts.set_transport(transport);
    opts
}
pub(crate) struct Conn<const N: usize = 32> {
    pub(crate) eventloop: EventLoop,
}
impl<const N: usize> Conn<N> {
    pub(crate) fn new(cfg: Config) -> (Self, AsyncClient) {
        let (cli, eventloop) = AsyncClient::new(cfg, N);
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
                    log::trace!("mqtt poll error Io :{}", e);
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
                _ => {
                    log::error!("mqtt poll error:{}----->", e);
                    return Some(MqttEventData::Error(format!("mqtt poll error:{}", e)));
                }
            }
        }

        let event = match event {
            Ok(v) => v,
            Err(e) => {
                log::error!("mqtt poll error:{}", e);
                return None;
            }
        };

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
                    Incoming::SubAck(s) => {
                        for r in s.return_codes {
                            match r {
                                SubscribeReasonCode::Success(_) => {
                                    log::debug!(
                                        "[incoming]-SubAck mqtt sub ack success pkid: {}",
                                        s.pkid
                                    );
                                }
                                _ => {
                                    log::error!(
                                        "[incoming]-SubAck mqtt sub ack error pkid: {} reason_code: {:?}",
                                        s.pkid,
                                        r
                                    );
                                }
                            }
                        }
                    }

                    Incoming::Publish(d) => {
                        log::trace!("[incoming]-Publish mqtt publish:{:?}", d);
                        return Some(MqttEventData::IncomeMsg(d));
                    }
                    Incoming::PubAck(s) => match s.reason {
                        PubAckReason::Success => {
                            log::trace!("[incoming]-PubAck mqtt pub ack success pkid: {}", s.pkid);
                        }
                        _ => {
                            log::error!(
                                "[incoming]-PubAck mqtt pub ack err  pkid: {}  reason_code:{:?}",
                                s.pkid,
                                s.reason
                            );
                        }
                    },

                    Incoming::PubRec(s) => {
                        log::debug!("[incoming]-PubRec {:?}", s);
                    }
                    Incoming::PubRel(s) => {
                        log::debug!("[incoming]-PubRel {:?}", s);
                    }
                    Incoming::PubComp(s) => {
                        log::debug!("[incoming]-PubComp {:?}", s);
                    }

                    Incoming::Connect(s, _, _) => {
                        log::debug!("[incoming]-Connnect mqtt connect:{:?}", s);
                    }

                    Incoming::ConnAck(d) => {
                        log::trace!("[incoming]-ConnAck mqtt conn ack: {:?}", d);
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
                        log::trace!("[incoming]-pingreq recv mqtt broker pong");
                    }
                    //服务器回复pong
                    Incoming::PingResp(_) => {
                        log::trace!("[incoming]-pingresp recv mqtt broker pong");
                    }
                    _ => {
                        log::debug!("[incoming] msg: {:?}", msg);
                    }
                }
            }
            Event::Outgoing(o) => match o {
                Outgoing::PingReq => {
                    log::trace!("[outgoing] send mqtt broker ping");
                }
                Outgoing::Publish(p) => {
                    log::trace!("[outgoing] publish packId:{}", p);
                }
                Outgoing::Subscribe(p) => {
                    log::trace!("[outgoing] subscribe packId:{}", p);
                }
                Outgoing::Unsubscribe(p) => {
                    log::trace!("[outgoing] unsubscribe packId:{}", p);
                }
                _ => log::trace!("[outgoing] msg:{:?}", o),
            },
        }

        None
    }
}
