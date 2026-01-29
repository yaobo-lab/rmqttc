#![allow(dead_code)]
use crate::{Config, MqttEventData};
use rumqttc::Outgoing;
use rumqttc::v5::mqttbytes::v5::{ConnectReturnCode, PubAckReason, SubscribeReasonCode};
use rumqttc::v5::{AsyncClient, ConnectionError, Event, EventLoop, Incoming, StateError};
use std::fmt::Debug;
use std::fmt::Formatter;

impl Debug for Conn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn").field("conf", &"conn").finish()
    }
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
            Event::Incoming(msg) => match msg {
                Incoming::Disconnect(s) => {
                    log::debug!("[incoming]-Disconnect reason:{:?}", s.reason_code);
                    return Some(MqttEventData::Disconnected);
                }

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

                Incoming::PingReq(_) => {
                    log::trace!("[incoming]-pingreq recv mqtt broker pong");
                }

                Incoming::PingResp(_) => {
                    log::trace!("[incoming]-pingresp recv mqtt broker pong");
                }
                _ => {
                    log::debug!("[incoming] msg: {:?}", msg);
                }
            },
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
