extern crate data_management;
extern crate sensor_interface;
extern crate mqtt;
extern crate clap;
extern crate time;
extern crate uuid;
extern crate json;
extern crate chrono;
extern crate env_logger;
extern crate rusqlite;
#[macro_use]
extern crate log;

use std::time::{SystemTime, UNIX_EPOCH};
use data_management::data_management::{data_base, DeviceData};
use sensor_interface::file_if;
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
use chrono::prelude::Local;
use std::sync::mpsc;

#[derive(Debug)]
enum SnMsgHandleError{
    SnMsgParseError,
    //SnMsgConnError,
    SnMsgPubError,
    //SnMsgDisconnError,
    SnMsgPacketError,
    SnMsgTopicNameError,
}

enum DbOp {
    INSERT,
    QUERY,
    DELETE,
}
struct DbReq {
    operation: DbOp,
    id: u32,
    data: DeviceData,
}

enum NetworkChange {
    ESTABILISH,
    LOSE,
}

struct NetworkNotify {
    stream: Option<TcpStream>,
    event: NetworkChange,
}

fn generate_client_id() -> String {
    format!("/MQTT/rust/{}", Uuid::new_v4())
}

fn init_data_base(path: &str, name: &str) -> Result<rusqlite::Connection, ()> {
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
/*
fn pub_sn_msg(sn: &str, topic: &str, msg: &str, server: &str) -> Result<(), SnMsgHandleError>
{
    // 连接服务器
    //info!("Connecting to {:?} ... ", server);
    let mut stream = match TcpStream::connect(server){
        Ok(stream) => stream,
        Err(_err) => return Err(SnMsgHandleError::SnMsgConnError),
    };
    //info!("Connected!");

    //info!("Client identifier {:?}", sn);
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
    //trace!("CONNACK {:?}", connack);

    if connack.connect_return_code() != ConnectReturnCode::ConnectionAccepted {
        //info!(
        //    "Failed to connect to server, return code {:?}",
        //    connack.connect_return_code()
        //);
        //return Err(SnMsgHandleError::SnMsgConnAckError);
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
*/
fn pub_sn_msg_use_stream(topic: &str, msg: &str, stream: &mut TcpStream) -> Result<(), SnMsgHandleError>
{
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
    Ok(())
}

fn get_data_from_msg(msg: &str) -> Result<DeviceData, SnMsgHandleError> {
    let parsed = match json::parse(&msg) {
        Ok(parsed) => parsed,
        Err(_err) => {
            return Err(SnMsgHandleError::SnMsgParseError);
        },
    };
    let n = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n,
        Err(_) => Duration::from_secs(0),
    };
    let timestamp_msec = n.as_millis() as i64;
    let time_string = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = match parsed["id"].as_u32() {
        Some(id) => id,
        None => return Err(SnMsgHandleError::SnMsgParseError),
    };
    let temperature = match parsed["temperature"].as_f64() {
        Some(t) => t,
        None => return Err(SnMsgHandleError::SnMsgParseError),
    };
    let humidity = match parsed["humidity"].as_f64() {
        Some(h) => h,
        None => return Err(SnMsgHandleError::SnMsgParseError),
    };
    let voltage = match parsed["voltage"].as_f64() {
        Some(v) => v,
        None => return Err(SnMsgHandleError::SnMsgParseError),
    };
    let status = match parsed["status"].as_i32() {
        Some(s) => s,
        None => return Err(SnMsgHandleError::SnMsgParseError),
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
    Ok(data)
}

fn get_msg_from_data(data: &DeviceData) -> String {
    let msg_json = json::object!{
        id: data.device_serial_number,
        temperature: data.temperature,
        humidity: data.humidity,
        voltage: data.voltage,
        status: data.error_code
    };
    msg_json.dump()
}

fn connect_to_server(server: &str, client_id: &str, channel_filters: &Vec<(TopicFilter, QualityOfService)>) -> Result<TcpStream, ()> {
    let keep_alive = 10;

    let mut stream = match TcpStream::connect(server) {
        Ok(stream) => stream,
        Err(_err) => return Err(()),
    };
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
        //panic!(
        //    "Failed to connect to server, return code {:?}",
        //    connack.connect_return_code()
        //);
        return Err(());
    }

    // const CHANNEL_FILTER: &'static str = "typing-speed-test.aoeu.eu";
    info!("Applying channel filters {:?} ...", channel_filters);
    let sub = SubscribePacket::new(10, channel_filters.to_vec());
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
    Ok(stream)
}

fn main() {
    env::set_var("RUST_LOG", env::var_os("RUST_LOG").unwrap_or_else(|| "info".into()));
    env_logger::init();

    let db = match init_data_base("./", "test.db") {
        Ok(database) => database,
        Err(err) => {
            panic!("Problem opening the database: {:?}", err)
        },
    };
    
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
    let mut try_to_connect = true;

    let (insert_req, db_handle) = mpsc::channel();
    let query_req =  mpsc::Sender::clone(&insert_req);
    let (db_query_rep_tx, db_query_rep_rx) = mpsc::channel();
    let db_delete_req_tx =  mpsc::Sender::clone(&insert_req);
    let (original_data_pub_strem_tx, original_data_pub_strem_rx) = mpsc::channel();
    let (offine_data_pub_stream_tx, offine_data_pub_stream_rx) = mpsc::channel();
    let (db_delete_rep_tx, db_delete_rep_rx) = mpsc::channel();

    // 从文件中获取传感器数据，如果和上次的不相同则处理
    thread::spawn(move || {
        let topic = String::from("sn_data");
        let mut last_msg = String::from("");
        let mut original_data_pub_strem_option: Option<TcpStream> = None;
        let mut network_ok = false;
        loop {
            match file_if::read_msg("./msg_data.txt") {
                Ok(sn_msg) => {
                    if last_msg.ne(&sn_msg) && !sn_msg.is_empty() {
                        match original_data_pub_strem_rx.try_recv() {
                            Ok(notification) => {
                                let notification: NetworkNotify = notification;
                                match notification.event {
                                    NetworkChange::ESTABILISH => {
                                        match notification.stream {
                                            Some(stream) => {
                                                network_ok = true;
                                                original_data_pub_strem_option = Some(stream);
                                            },
                                            _ => {},
                                        };
                                    },
                                    NetworkChange::LOSE => {
                                        network_ok = false;
                                    },
                                }
                            },
                            Err(_err) => {},
                        }
                        if network_ok {
                            match original_data_pub_strem_option {
                                Some(ref mut stream) => {
                                    let r = pub_sn_msg_use_stream(&topic, &sn_msg, stream);
                                    match r {
                                        Ok(_ok) => {
                                            info!("pub msg successfully: {}", &sn_msg);
                                            continue;
                                            },
                                        Err(err) => error!("pub msg failed: {:#?}", err),
                                    }
                                },
                                None => {},
                            }
                        }
                        let device_data = match get_data_from_msg(&sn_msg) {
                            Ok(data) => data,
                            Err(_err) => {
                                error!("parse {} failed", &sn_msg);
                                continue;
                            },
                        };
                        let db_req = DbReq{
                            operation: DbOp::INSERT,
                            id: 0,
                            data: device_data,
                        };
                        match insert_req.send(db_req) {
                            Err(err) => {
                                error!("send insert req failed: {}", err);
                                continue;
                            },
                            _ => {},
                        }
                        last_msg = sn_msg.clone();
                    }
                },
                Err(_err) => {},
            };
            thread::sleep(Duration::from_secs(1));
        }
    });

    //离线数据处理
    thread::spawn(move || {
        let mut offine_data_pub_stream_option: Option<TcpStream> = None;
        loop {
            match offine_data_pub_stream_rx.recv() {
                Ok(notification) => {
                    let notification: NetworkNotify = notification;
                    match notification.event {
                        NetworkChange::ESTABILISH => {
                            match notification.stream {
                                Some(stream) => {
                                    offine_data_pub_stream_option = Some(stream);
                                },
                                _ => {},
                            };
                        },
                        NetworkChange::LOSE => {
                            continue;
                        },
                    }
                },
                Err(_err) => {},
            }
            let db_req = DbReq{
                operation: DbOp::QUERY,
                id: 0,
                data: DeviceData::new(0, 0, "", 0.0, 0.0, 0.0, 0, 0),
            };
            match query_req.send(db_req) {
                Ok(_ok) => {
                    match db_query_rep_rx.recv() {
                        Ok(vec) => {
                            for tuple in vec {
                                let (id, device_data) = match tuple {
                                    Ok(t) => t,
                                    Err(err) => {
                                        error!("get data from data iter failed: {}", err);
                                        continue;
                                    },
                                };
                                let topic = String::from("sn_data");
                                let sn_msg = get_msg_from_data(&device_data);
                                match offine_data_pub_stream_option {
                                            Some(ref mut stream) => {
                                                if let Ok(_) = pub_sn_msg_use_stream(&topic, &sn_msg, stream) {
                                                    //数据上传成功，删除数据库中对应的记录
                                                    let db_delete_req = DbReq{
                                                        operation: DbOp::DELETE,
                                                        id: id,
                                                        data: DeviceData::new(0, 0, "", 0.0, 0.0, 0.0, 0, 0),
                                                    };
                                                    match db_delete_req_tx.send(db_delete_req) {
                                                        Ok(_ok) => {
                                                            match db_delete_rep_rx.recv() {
                                                                Ok(r) => {
                                                                    if r {
                                                                        info!("handle offline data successfully");
                                                                    } else {
                                                                        error!("delete offline data after publishing failed");
                                                                    }
                                                                },
                                                                Err(_err) => {},
                                                            }
                                                        },
                                                        Err(err) => {
                                                            error!("send offline data delete req failed: {}", err);
                                                        },
                                                    }
                                                }
                                            },
                                            None => {},
                                }
                                thread::sleep(Duration::from_millis(100));
                            }
                        },
                        Err(_err) => {},
                    }
                },
                Err(_err) => {},
            }
        }
    });

    //数据库操作
    let _db_handle = thread::spawn( move || {
        loop {
            let db_req = match db_handle.recv() {
                Ok(req) => req,
                Err(_err) => continue,
            };
            match db_req.operation {
                DbOp::INSERT => {
                    match data_base::insert_data_to_device_data_table(&db, &db_req.data) {
                        Ok(_ok) => {
                            info!("buffed data successfully");
                        },
                        Err(err) => {
                            error!("buffed data  failed: {:?}", err);
                        },
                    }
                },
                DbOp::QUERY => {
                    let mut stmt = match data_base::querry_device_data(&db) {
                        Ok(stmt) => stmt,
                        Err(_err) => {
                            error!("querry database failed");
                            continue;
                        },
                    };
                    let data_iter = match stmt.query_map(rusqlite::params![], |row| {
                        let id: u32 = row.get(0)?;
                        let data = DeviceData {
                            device_serial_number: row.get(1)?,
                            timestamp_msec: row.get(2)?,
                            time_string: row.get(3)?,
                            temperature: row.get(4)?,
                            humidity: row.get(5)?,
                            voltage: row.get(6)?,
                            rssi: 0,
                            error_code: 0,
                        };
                        Ok(
                            (id, data)
                        )
                    }) {
                        Ok(iter) => iter,
                        Err(_err) => {
                            error!("get data iter failed");
                            continue;
                        },
                    };
                    let mut vec = Vec::new();
                    for tuple in data_iter {
                        vec.push(tuple);
                    }
                    match db_query_rep_tx.send(vec) {
                        _ => {},
                    }
                },
                DbOp::DELETE => {
                    match data_base::delete_device_data(&db, db_req.id){
                        Ok(_ok) => {
                            match db_delete_rep_tx.send(true) {
                                Ok(_ok) => {},
                                Err(err) => error!("send delete rep failed: {}", err),
                            }
                        },
                        Err(_err) => {
                            match db_delete_rep_tx.send(false) {
                                Ok(_ok) => {},
                                Err(err) => error!("send delete rep failed: {}", err),
                            }
                        },
                    }
                },
            }
    
        }
    });

    loop {
        if try_to_connect {
            info!("Connecting to {:?} ... ", server_addr);
            try_to_connect = false;
        }
        let mut stream = match connect_to_server(&server_addr, &client_id, &channel_filters) {
            Ok(stream) => stream,
            Err(_err) => {
                thread::sleep(Duration::new(10, 0));
                continue;
            },
        };

        let (main_thread_tx, ping_thread_rx) = mpsc::channel();
        let mut stream_clone = stream.try_clone().unwrap();

        match stream.try_clone() {
            Ok(original_data_pub_strem) => {
                let msg = NetworkNotify{
                    stream: Some(original_data_pub_strem),
                    event: NetworkChange::ESTABILISH,
                };
                match original_data_pub_strem_tx.send(msg) {
                    Ok(_ok) => {},
                    Err(err) => error!("send NetworkNotify failed: {}", err),
                }
            },
            Err(err) => error!("clone original_data_pub_strem failed: {}", err),
        }

        match stream.try_clone() {
            Ok(offine_data_pub_stream) => {
                let msg = NetworkNotify{
                    stream: Some(offine_data_pub_stream),
                    event: NetworkChange::ESTABILISH,
                };
                match offine_data_pub_stream_tx.send(msg) {
                    Ok(_ok) => {},
                    Err(err) => error!("send NetworkNotify failed: {}", err),
                }
            },
            Err(err) => error!("clone offine_data_pub_stream failed: {}", err),
        }

        let ping_thread = thread::spawn(move || {
            let keep_alive = 10;
            let mut last_ping_time = 0;
            let mut next_ping_time = last_ping_time + (keep_alive as f32 * 0.9) as i64;
            loop {
                match ping_thread_rx.try_recv() {
                    Ok(network_change) => {
                        match network_change {
                            NetworkChange::LOSE => {
                                break;
                            },
                            _ => {},
                        }
                    },
                    _ => {},
                };
                let current_timestamp = time::get_time().sec;
                if keep_alive > 0 && current_timestamp >= next_ping_time {
                    //info!("Sending PINGREQ to broker");
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
                Err(_err) => {
                    //error!("Error in receiving packet {}", err);
                    match main_thread_tx.send(NetworkChange::LOSE) {
                        Ok(_ok) => {},
                        Err(_err) => {},
                    }
                    match ping_thread.join() {
                        _ => {},
                    }
                    break;
                }
            };
            //trace!("PACKET {:?}", packet);

            match packet {
                VariablePacket::PingrespPacket(..) => {
                    //info!("Receiving PINGRESP from broker ..");
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
                }
                _ => {}
            }
        }
        let msg = NetworkNotify{
            stream: None,
            event: NetworkChange::LOSE,
        };
        match offine_data_pub_stream_tx .send(msg) {
            Ok(_ok) => {},
            Err(err) => error!("send NetworkNotify failed: {}", err),
        }
        let msg = NetworkNotify{
            stream: None,
            event: NetworkChange::LOSE,
        };
        match original_data_pub_strem_tx.send(msg) {
            Ok(_ok) => {},
            Err(err) => error!("send NetworkNotify failed: {}", err),
        }
        error!("lose connection to {}", &server_addr);
        try_to_connect = true;
    }
}
