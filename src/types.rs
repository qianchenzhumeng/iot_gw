extern crate paho_mqtt;
use serde_derive::Deserialize;
use std::sync::mpsc::Receiver;

pub type MsgReceiver = Receiver<Option<paho_mqtt::Message>>;

#[derive(Deserialize)]
pub struct ClientConfig {
    pub id: String,
    pub keep_alive: u16,
    pub username: String,
}

#[derive(Deserialize)]
pub struct TopicConfig {
    pub sub_topic: String,
    pub pub_topic: String,
    pub pub_log_topic: String,
    pub qos: i32,
}

#[derive(Deserialize)]
pub struct TlsFiles {
    pub cafile: String,
    pub key_store: String,
}