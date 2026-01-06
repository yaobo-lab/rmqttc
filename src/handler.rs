use crate::MqttEvent;
pub use rumqttc::v5::mqttbytes::v5::Publish as Message;
pub trait IHandler: Send + Sync + 'static {
    fn on_event(&self, event: MqttEvent);
    fn on_message(&self, msg: Message);
}
