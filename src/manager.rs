use crate::{
    Config, Conn, MqttEvent, MqttEventData, MqttMsg, OnEventCallback, OnMessageCallback, State,
};
use anyhow::{Result, anyhow};
use rumqttc::v5::AsyncClient;
use std::{collections::HashMap, time::Duration};
use tokio::{
    select,
    sync::{mpsc, watch},
    time::sleep,
};

pub struct Manager {
    pub state: watch::Sender<State>,
    pub(crate) conf: Config,
    pub(crate) topics: HashMap<String, crate::QoS>,
    pub(crate) cli: Option<AsyncClient>,
    pub(crate) on_msg: OnMessageCallback,
    pub(crate) on_event: OnEventCallback,
}

impl Manager {
    pub(crate) fn new(
        conf: Config,
        state: watch::Sender<State>,
        on_msg: OnMessageCallback,
        on_event: OnEventCallback,
        topics: HashMap<String, crate::QoS>,
    ) -> Self {
        Manager {
            conf,
            cli: None,
            state,
            topics,
            on_msg,
            on_event,
        }
    }
    fn add_topic(&mut self, topic: &str, qos: crate::QoS) {
        self.topics.insert(topic.to_string(), qos);
    }

    fn remove_topic(&mut self, topic: &str) {
        self.topics.remove(topic);
    }
    //关闭客户端
    async fn close(&self) -> Result<()> {
        if let Some(ref cli) = self.cli {
            cli.disconnect().await?;
        }
        Ok(())
    }

    // 创建mqtt 连接
    fn crate_conn(&mut self) -> Conn {
        let (conn, cli) = Conn::new(self.conf.clone());
        self.cli = Some(cli);
        conn
    }
    //初始化订阅
    async fn init_sub_topic(&self) -> Result<()> {
        let Some(ref cli) = self.cli else {
            return Err(anyhow!("cli is None"));
        };
        for (topic, qos) in self.topics.iter() {
            cli.subscribe(topic, qos.clone()).await?;
        }
        Ok(())
    }

    //运行
    pub(crate) async fn run(&mut self, mut cmd_recv: mpsc::Receiver<MqttMsg>) {
        let mut conn = self.crate_conn();
        loop {
            select! {
               s=conn.poll_msg()=>{

                let Some(s) = s else{
                    continue;
                };

                match s {
                     MqttEventData::Disconnected=>{
                       if *self.state.borrow() != State::Disconnected{
                           (self.on_event)(MqttEvent::Disconnected);
                           self.state.send(State::Disconnected).ok();
                       }
                       //断线重连
                       sleep(Duration::from_secs(10)).await;
                       conn=self.crate_conn();
                    },
                    MqttEventData::Connected=>{
                         log::info!("mqtt connected");
                        (self.on_event)(MqttEvent::Connected);
                        self.state.send(State::Connected).ok();
                        //初始化订阅主题
                        if let Err(e)= self.init_sub_topic().await{
                            log::error!("init sub topic err:{}",e);
                        }
                    },
                    MqttEventData::Error(e)=>{
                        self.state.send(State::Error(e.to_string())).ok();
                        (self.on_event)(MqttEvent::Error(e.to_string()));
                        break;
                    }
                    MqttEventData::IncomeMsg(msg)=>{
                        (self.on_msg)(msg);
                    }
                 }
                }

               cmd= cmd_recv.recv()=>{
                  let Some(cmd) = cmd else{
                     continue;
                  };


                if cmd==MqttMsg::Closed{
                     if let Err(e)=self.close().await{
                        log::error!("close error:{}", e);
                     }
                     self.state.send(State::Closed).ok();
                     (self.on_event)(MqttEvent::Closed);
                     break;
                 }

                 let Some(ref cli)=self.cli else{
                     log::warn!("mqtt client not init revc cmd :{:?}",cmd);
                     continue;
                 };



                  match cmd {
                        MqttMsg::Sub(cmd) => {
                          if let Err(e)= cli.subscribe(cmd.topic.clone(), cmd.qos).await{
                               log::error!("mqtt subscribe topic:{} error:{}",cmd.topic, e);
                          }else{
                              self.add_topic(&cmd.topic, cmd.qos);
                          }
                        }
                        MqttMsg::UnSub(topic) => {
                           if let Err(e)= cli.unsubscribe(topic.clone()).await{
                              log::error!("mqtt unsubscribe topic:{} error:{}",topic, e);
                           }
                           self.remove_topic(&topic);
                        }
                        MqttMsg::Pub(cmd) => {
                            if let Err(e)= cli.publish(cmd.topic.clone(), cmd.qos, cmd.retain, cmd.data.to_string())
                                .await{
                                    log::error!("publish:{} error:{}",cmd.topic, e);
                               }
                        }
                        _ => {}
                      }
                    }
            }
        }

        log::error!("mqtt manager run loop exit...");
    }
}
