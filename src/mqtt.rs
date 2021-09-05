pub mod closure {
    extern crate paho_mqtt;
    use std::sync::mpsc::{Sender, Receiver};
    use std::time::Duration;
    use std::{process, thread};
    use crate::types::{ClientConfig, TopicConfig, MsgReceiver, TlsFiles};
    use log::{error, warn, info, debug, LevelFilter};

    pub fn pub_closure(client: ClientConfig, topic: TopicConfig, tls: TlsFiles, cloud_statue_announcement_sender: Sender<Option<u8>>,
        tx: Sender<MsgReceiver>, datum_publish_receiver: Receiver<Option<String>>, format_log: fn(msg: &str) -> Result<String, ()>,
        publish_result_sender: Sender<bool>, mqtt_message_receiver: Receiver<paho_mqtt::Message>, server_addr: String
    ) -> impl FnOnce() -> () {
        move || {
            let create_opts = paho_mqtt::CreateOptionsBuilder::new()
                .server_uri(server_addr)
                .client_id(client.id)
                .max_buffered_messages(1) // 离线时不缓存数据
                .finalize();

            let mut cli = paho_mqtt::Client::new(create_opts).unwrap_or_else(|e| {
                error!("Error creating the client: {:?}", e);
                process::exit(1);
            });

            cli.set_timeout(Duration::from_secs(5));
            let sub_msg_receiver = cli.start_consuming();

            #[cfg(feature = "ssl")]
            let ssl_opts = paho_mqtt::SslOptionsBuilder::new()
                .trust_store(&tls.cafile).unwrap()
                .key_store(&tls.key_store).unwrap()
                .finalize();

            #[cfg(feature = "ssl")]
            let conn_opts = paho_mqtt::ConnectOptionsBuilder::new()
                .ssl_options(ssl_opts)
                .keep_alive_interval(Duration::from_secs(client.keep_alive.into()))
                .mqtt_version(paho_mqtt::MQTT_VERSION_3_1_1)
                .clean_session(true)
                .user_name(client.username)
                .finalize();

            #[cfg(not(feature = "ssl"))]
            let conn_opts = paho_mqtt::ConnectOptionsBuilder::new()
                .keep_alive_interval(Duration::from_secs(client.keep_alive.into()))
                .mqtt_version(paho_mqtt::MQTT_VERSION_3_1_1)
                .clean_session(true)
                .user_name(client.username)
                //.automatic_reconnect(Duration::from_secs(1), Duration::from_secs(30))
                .finalize();

            info!("Connecting to the MQTT broker...");
            match cli.connect(conn_opts) {
                Ok(rsp) => {
                    if let Some(cr) = rsp.connect_response() {
                        if let Err(err) = cloud_statue_announcement_sender.send(Some(0)) {
                            error!("Error send cloud statue announcement: {}", err);
                        }
                        info!("Connected to: '{}' with MQTT version {}", cr.server_uri, cr.mqtt_version);
                        // Register subscriptions on the server
                        debug!("Subscribing to topics, with requested QoS: {:?}...", topic.qos);
                        match cli.subscribe(&topic.sub_topic, topic.qos) {
                            Ok(qosv) => debug!("QoS granted: {:?}", qosv),
                            Err(e) => {
                                debug!("Error subscribing to topics: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error connecting to the broker: {:?}", e);
                    loop {
                        if cli.reconnect().is_ok() {
                            if let Err(err) = cloud_statue_announcement_sender.send(Some(0)) {
                                error!("Error send cloud statue announcement: {}", err);
                            }
                            // Register subscriptions on the server
                            debug!("Subscribing to topics, with requested QoS: {:?}...", topic.qos);
                            match cli.subscribe(&topic.sub_topic, topic.qos) {
                                Ok(qosv) => debug!("QoS granted: {:?}", qosv),
                                Err(e) => {
                                    debug!("Error subscribing to topics: {:?}", e);
                                }
                            }
                            break;
                        } else {
                            thread::sleep(Duration::from_secs(10));
                        }
                    }
                }
            }

            match tx.send(sub_msg_receiver) {
                Err(err) => error!("Send msg receiver failed: {}", err),
                _ => {}
            }
            loop {
                let publish_result: bool;
                match datum_publish_receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(option) => if let Some(msg) = option {
                        let message = paho_mqtt::Message::new(topic.pub_topic.clone(), msg, topic.qos);
                        debug!("message: {}", message);
                        if let Err(e) = cli.publish(message) {
                            error!("Error publishing message: {:?}", e);
                            publish_result = false;
                        } else {
                            publish_result = true;
                            // 数据发布成功后发送 LOG
                            match format_log("") {
                                Ok(log) => {
                                    let log_msg = paho_mqtt::Message::new(topic.pub_log_topic.clone(), log, topic.qos);
                                    match cli.publish(log_msg) {
                                        Err(err) => error!("Error publishing log: {:?}", err),
                                        _ => {}
                                    }
                                },
                                Err(err) => error!("Error formating log: {:?}", err), 
                            }
                        }
                        match publish_result_sender.send(publish_result) {
                            Err(err) => error!("Error send publish result: {}", err),
                            _ => {},
                        }
                    }
                    Err(_) => {},
                }
                // 发布其他线程发过来的 MQTT 消息
                if let Ok(mqtt_message) = mqtt_message_receiver.try_recv() {
                    if let Err(e) = cli.publish(mqtt_message) {
                        error!("Error publishing message: {:?}", e);
                    }
                }
                if !cli.is_connected() {
                    if cli.reconnect().is_ok() {
                        if let Err(err) = cloud_statue_announcement_sender.send(Some(0)) {
                            error!("Error send cloud statue announcement: {}", err);
                        }
                        // Register subscriptions on the server
                        debug!("Subscribing to topics, with requested QoS: {:?}...", topic.qos);
                        match cli.subscribe(&topic.sub_topic, topic.qos) {
                            Ok(qosv) => debug!("QoS granted: {:?}", qosv),
                            Err(e) => {
                                debug!("Error subscribing to topics: {:?}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn sub_closure(rx: Receiver<MsgReceiver>, downstream_msg_tx: Sender<String>,
        cloud_statue_announcement_sender: Sender<Option<u8>>) -> impl FnOnce() -> () {
        move || loop {
            match rx.recv() {
                Ok(r) => {
                    for msg in r.iter() {
                        if let Some(msg) = msg {
                            if let Err(err) = downstream_msg_tx.send(String::from(msg.payload_str()))
                            {
                                error!("Send downstream msg failed(err: {}, msg: {})", err, msg);
                            }
                        } else {
                            if let Err(err) = cloud_statue_announcement_sender.send(None) {
                                error!("Error send cloud statue announcement: {}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("mqtt_sub_thread recv error: {}", err);
                }
            }
        }
    }

}