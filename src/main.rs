extern crate data_management;
extern crate mqtt;
extern crate clap;
extern crate time;
extern crate uuid;
extern crate threadpool;
extern crate json;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate rusqlite;
extern crate chrono;

use std::time::{SystemTime, UNIX_EPOCH};
use data_management::data_management::{data_base, DeviceData};
use std::env;
use std::io::Write;
use std::net::TcpStream;
use std::str;
use clap::{App, Arg};
use uuid::Uuid;
use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::*;
use mqtt::{TopicFilter, TopicName};
use mqtt::{Decodable, Encodable, QualityOfService};
use std::thread;
use std::time::Duration;
use threadpool::ThreadPool;
use chrono::prelude::*;
use std::sync::{Arc, Mutex};

fn generate_client_id() -> String {
    format!("/MQTT/rust/{}", Uuid::new_v4())
}

fn init_data_base(path: &str, name: &str) -> Result<Arc<Mutex<rusqlite::Connection>>, ()> {
    let db = data_base::open_data_base(path, name);

    let db = match db {
        Ok(database) => database,
        Err(err) => {
            panic!("Problem opening the database: {:?}", err)
        },
    };

    match data_base::create_device_table(&db) {
        Ok(_ok) => info!("create DEVICE table successfully"),
        Err(err) => error!("create DEVICE table failed: {:?}", err),
    }

    match data_base::create_device_data_table(&db) {
        Ok(_ok) => info!("create DEVICE_DATA table successfully"),
        Err(err) => error!("create DEVICE_DATA table failed: {:?}", err),
    }

    match data_base::create_device_error_table(&db) {
        Ok(_ok) => info!("create DEVICE_ERROR table successfully"),
        Err(err) => error!("create DEVICE_ERROR table failed: {:?}", err),
    }

    Ok(db)
}

enum SnMsgHandleError{
    SnMsgParseError,
    SnMsgConnError,
    SnMsgConnAckError,
    SnMsgPubError,
    SnMsgDisconnError,
    SnMsgPacketError,
    SnMsgTopicNameError,
    SnMsgBuffDataError,
}

fn sn_msg_pub(sn: &str, topic: &str, msg: &str, server: &str) -> Result<(), SnMsgHandleError>
{
    // 连接服务器
    info!("Connecting to {:?} ... ", server);
    let mut stream = match TcpStream::connect(server){
        Ok(stream) => stream,
        Err(_err) => return Err(SnMsgHandleError::SnMsgConnError),
    };
    info!("Connected!");

    info!("Client identifier {:?}", sn);
    let mut conn = ConnectPacket::new("MQTT", sn);
    conn.set_clean_session(true);
    let mut buf = Vec::new();
    match conn.encode(&mut buf) {
        Ok(_) => {},
        Err(_err) => return Err(SnMsgHandleError::SnMsgPacketError),
    };
    match stream.write_all(&buf[..]) {
        Ok(_) => {},
        Err(_err) => return Err(SnMsgHandleError::SnMsgConnError),
    };

    let connack = match ConnackPacket::decode(&mut stream) {
        Ok(connack) => connack,
        Err(_) => return Err(SnMsgHandleError::SnMsgPacketError),
    };
    trace!("CONNACK {:?}", connack);

    if connack.connect_return_code() != ConnectReturnCode::ConnectionAccepted {
        info!(
            "Failed to connect to server, return code {:?}",
            connack.connect_return_code()
        );
        return Err(SnMsgHandleError::SnMsgConnAckError);
    }
    // 发布消息
    let topic_name = match TopicName::new(topic) {
        Ok(topic_name) => topic_name,
        Err(_)  => return Err(SnMsgHandleError::SnMsgTopicNameError),
    };
    let publish_packet = PublishPacket::new(topic_name, QoSWithPacketIdentifier::Level0, msg.clone());
    let mut buf = Vec::new();
    match publish_packet.encode(&mut buf) {
        Ok(_) => {},
        Err(_) => return Err(SnMsgHandleError::SnMsgPacketError),
    };
    match stream.write_all(&buf[..]) {
        Ok(_) => {},
        Err(_err) => return Err(SnMsgHandleError::SnMsgPubError),
    };
    // 断开连接
    let disconn_pakdet = DisconnectPacket::new();
    let mut buf = Vec::new();
    match disconn_pakdet.encode(&mut buf) {
        Ok(_) => {},
        Err(_err) => return Err(SnMsgHandleError::SnMsgPacketError),
    };
    match stream.write_all(&buf[..]) {
        Ok(_) => {},
        Err(_err) => return Err(SnMsgHandleError::SnMsgDisconnError),
    };
    Ok(())
}

fn sn_msg_handle(sn: &str, topic: &str, msg: &str, server: &str) -> Result<(), SnMsgHandleError>
{
    let parsed = match json::parse(&msg) {
        Ok(parsed) => parsed,
        Err(_err) => {
            error!("bad JSON string from sn {}: {}", sn, &msg);
            return Err(SnMsgHandleError::SnMsgParseError);
        },
    };
    if let Err(_) = sn_msg_pub(&sn, &topic,&msg, server) {
        error!("publish sn {} msg failed: {}", &sn, &msg);
        let n = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(n) => n,
            Err(_) => Duration::from_secs(0),
        };
        let timestamp_msec = n.as_millis() as u64;
        let time_string = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let id = match parsed["id"].as_u32() {
            Some(id) => id,
            None => 0,
        };
        let temperature = match parsed["temperature"].as_f32() {
            Some(t) => t,
            None => 0.0,
        };
        let humidity = match parsed["humidity"].as_f32() {
            Some(h) => h,
            None => 0.0,
        };
        let voltage = match parsed["voltage"].as_f32() {
            Some(v) => v,
            None => 0.0,
        };
        let status = match parsed["status"].as_i32() {
            Some(s) => s,
            None => 1,
        };
        let data = DeviceData{
            device_serial_number: id,
            timestamp_msec: timestamp_msec,
            time_string: time_string,
            temperature: temperature,
            humidity: humidity,
            voltage: voltage,
            rssi: 0,
            error_code: status,
        };
        let db = match data_base::open_data_base("./", "test.db") {
            Ok(db) => db,
            Err(err) => {
                error!("open database  failed: {:?}", err);
                return Err(SnMsgHandleError::SnMsgBuffDataError);
            },
        };
        match data_base::insert_data_to_device_data_table(&db, &data) {
            Ok(_ok) => {
                info!("buffed data successfully");
                return Ok(());
            },
            Err(err) => {
                error!("buffed data  failed: {:?}", err);
                return Err(SnMsgHandleError::SnMsgBuffDataError);
            },
        }
        //match data_base::close_data_base(&db) {
        //    Ok(_ok) => Ok(_ok) => info!("buffed data successfully"),
        //    Err(err) => {
        //        error!("Problem closing the database: {:?}", err)
        //    },
        //info!("data: {:#?}", data);
        return Err(SnMsgHandleError::SnMsgPubError);
    }
    Ok(())
}

fn main() {
    env::set_var("RUST_LOG", env::var_os("RUST_LOG").unwrap_or_else(|| "info".into()));
    env_logger::init();

    match init_data_base("./", "test.db") {
        Ok(database) => database,
        Err(err) => {
            panic!("Problem opening the database: {:?}", err)
        },
    };
    let thread_pool = ThreadPool::new(8);

    let matches = App::new("sub-client")
        .author("Y. T. Chung <zonyitoo@gmail.com>")
        .arg(
            Arg::with_name("SERVER")
                .short("S")
                .long("server")
                .takes_value(true)
                .required(true)
                .help("MQTT server address (host:port)"),
        ).arg(
            Arg::with_name("SUBSCRIBE")
                .short("s")
                .long("subscribe")
                .takes_value(true)
                .multiple(true)
                .required(true)
                .help("Channel filter to subscribe"),
        ).arg(
            Arg::with_name("USER_NAME")
                .short("u")
                .long("username")
                .takes_value(true)
                .help("Login user name"),
        ).arg(
            Arg::with_name("PASSWORD")
                .short("p")
                .long("password")
                .takes_value(true)
                .help("Password"),
        ).arg(
            Arg::with_name("CLIENT_ID")
                .short("i")
                .long("client-identifier")
                .takes_value(true)
                .help("Client identifier"),
        ).get_matches();

    let server_addr = matches.value_of("SERVER").unwrap();
    let client_id = matches
        .value_of("CLIENT_ID")
        .map(|x| x.to_owned())
        .unwrap_or_else(generate_client_id);
    let channel_filters: Vec<(TopicFilter, QualityOfService)> = matches
        .values_of("SUBSCRIBE")
        .unwrap()
        .map(|c| (TopicFilter::new(c.to_string()).unwrap(), QualityOfService::Level0))
        .collect();

    let keep_alive = 60;

    info!("Connecting to {:?} ... ", server_addr);
    let mut stream = TcpStream::connect(server_addr).unwrap();
    info!("Connected!");

    info!("Client identifier {:?}", client_id);
    let mut conn = ConnectPacket::new("MQTT", client_id);
    conn.set_clean_session(true);
    conn.set_keep_alive(keep_alive);
    let mut buf = Vec::new();
    conn.encode(&mut buf).unwrap();
    stream.write_all(&buf[..]).unwrap();

    let connack = ConnackPacket::decode(&mut stream).unwrap();
    trace!("CONNACK {:?}", connack);

    if connack.connect_return_code() != ConnectReturnCode::ConnectionAccepted {
        panic!(
            "Failed to connect to server, return code {:?}",
            connack.connect_return_code()
        );
    }

    // const CHANNEL_FILTER: &'static str = "typing-speed-test.aoeu.eu";
    info!("Applying channel filters {:?} ...", channel_filters);
    let sub = SubscribePacket::new(10, channel_filters);
    let mut buf = Vec::new();
    sub.encode(&mut buf).unwrap();
    stream.write_all(&buf[..]).unwrap();

    loop {
        let packet = match VariablePacket::decode(&mut stream) {
            Ok(pk) => pk,
            Err(err) => {
                error!("Error in receiving packet {:?}", err);
                continue;
            }
        };
        trace!("PACKET {:?}", packet);

        if let VariablePacket::SubackPacket(ref ack) = packet {
            if ack.packet_identifier() != 10 {
                panic!("SUBACK packet identifier not match");
            }

            info!("Subscribed!");
            break;
        }
    }

    let mut stream_clone = stream.try_clone().unwrap();
    thread::spawn(move || {
        let mut last_ping_time = 0;
        let mut next_ping_time = last_ping_time + (keep_alive as f32 * 0.9) as i64;
        loop {
            let current_timestamp = time::get_time().sec;
            if keep_alive > 0 && current_timestamp >= next_ping_time {
                info!("Sending PINGREQ to broker");

                let pingreq_packet = PingreqPacket::new();

                let mut buf = Vec::new();
                pingreq_packet.encode(&mut buf).unwrap();
                stream_clone.write_all(&buf[..]).unwrap();

                last_ping_time = current_timestamp;
                next_ping_time = last_ping_time + (keep_alive as f32 * 0.9) as i64;
                thread::sleep(Duration::new((keep_alive / 2) as u64, 0));
            }
        }
    });

    loop {
        let packet = match VariablePacket::decode(&mut stream) {
            Ok(pk) => pk,
            Err(err) => {
                //error!("Error in receiving packet {}", err);
                //continue;
                panic!("Error in receiving packet {}", err);
            }
        };
        trace!("PACKET {:?}", packet);

        match packet {
            VariablePacket::PingrespPacket(..) => {
                info!("Receiving PINGRESP from broker ..");
            }
            VariablePacket::PublishPacket(ref publ) => {
                let msg = match str::from_utf8(&publ.payload_ref()[..]) {
                    Ok(msg) => msg,
                    Err(err) => {
                        error!("Failed to decode publish message {:?}", err);
                        continue;
                    }
                };
                info!("PUBLISH ({}): {}", publ.topic_name(), msg);
                let mut sn_topic = Vec::with_capacity(4);
                let topic_name = String::from(&publ.topic_name()[..]);
                let v: Vec<&str> = topic_name.split('/').collect();
                // 不足 4 的时候应该会有问题
                for i in 0..4 {
                    sn_topic.push(String::from(v[i]));
                }
                if sn_topic[0] == "client" {
                    //let client_id = &sn_topic[1];
                    //let topic = &sn_topic[3];
                    let sn_msg = String::from(msg);
                    thread_pool.execute(move || {
                        let r = sn_msg_handle(&sn_topic[1], &sn_topic[3],
                                    &sn_msg, "127.0.0.1:1884");
                        match r {
                            Ok(_) => info!("handle sn msg successfully"),
                            Err(_) => error!("handle sn({}) msg failed", &sn_topic[1]),
                        };
                    });
                }
            }
            _ => {}
        }
    }
}
