extern crate data_management;
extern crate mqtt;
extern crate clap;
extern crate time;
extern crate uuid;
extern crate json;
extern crate chrono;
extern crate env_logger;
extern crate rusqlite;
extern crate toml;
extern crate serde_derive;
extern crate data_template;
#[cfg(feature = "data_interface_serial_port")]
extern crate serial;
#[cfg(feature = "data_interface_serial_port")]
extern crate hdtp;
#[cfg(feature = "data_interface_text_file")]
extern crate sensor_interface;
#[macro_use]
extern crate log;

use std::time::{SystemTime, UNIX_EPOCH, Duration};
use data_management::data_management::{data_base, DeviceData};

use std::{env, fs, thread, str};
use std::io::prelude::*;
use std::net::TcpStream;
use clap::{App, Arg};
use mqtt::control::variable_header::ConnectReturnCode;
use mqtt::packet::*;
use mqtt::{Decodable, Encodable, QualityOfService, TopicFilter, TopicName};
use chrono::prelude::Local;
use std::sync::mpsc;
use serde_derive::Deserialize;
use data_template::{Template, Model, Value};
#[cfg(feature = "data_interface_serial_port")]
use serial::prelude::*;
#[cfg(feature = "data_interface_serial_port")]
use hdtp::Hdtp;

#[cfg(feature = "data_interface_text_file")]
use sensor_interface::FileIf;
#[cfg(feature = "data_interface_text_file")]
use std::fs::File;

#[derive(Debug)]
enum SnMsgHandleError{
    SnMsgParseError,
    //SnMsgConnError,
    SnMsgPubError,
    //SnMsgDisconnError,
    SnMsgPacketError,
    SnMsgTopicNameError,
    //SnMsgConvertError,
}

#[derive(Debug)]
enum DataIfError{
    DataIfOpenError,
    DataIfInitError,
    DataIfUnknownType,
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

#[derive(Deserialize)]
struct Config {
    server: ServerConfig,
    client: ClientConfig,
    topic: TopicConfig,
    msg: MsgConfig,
    database: DatabaseConfig,
    data_if: DataIfConfig,
}

#[derive(Deserialize)]
struct ServerConfig {
    address: String,
}

#[derive(Deserialize)]
struct ClientConfig {
    id: String,
}

#[derive(Deserialize)]
struct TopicConfig {
    sub_topic: String,
    pub_topic: String,
}

#[derive(Deserialize)]
struct MsgConfig {
    example: String,
    template: String,
}

#[derive(Deserialize)]
struct DatabaseConfig {
    path: String,
    name: String,
}

#[derive(Deserialize)]
struct DataIfConfig {
    if_name: String,
    if_type: String,
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
    let name = match parsed["name"].as_str() {
        Some(name) => name.to_string(),
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
        name: name,
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
        name: data.name.clone(),
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

fn format_msg(original: &str, template_str: &str) -> Result<String, ()> {
    let parsed = match json::parse(&original) {
        Ok(parsed) => parsed,
        Err(_err) => return Err(()),
    };
    let template = Template::new(template_str);
    let mut msg = template_str.to_string();
    match template.get_value_models() {
        Ok(value_models) => {
            for model in value_models {
                match model.get_label() {
                    Ok(label) => {
                        //replace
                        let value_model = match model {
                            Model::Value(value_model) => value_model,
                        };
                        if parsed[&label].is_string() {
                            let string = match parsed[&label].as_str() {
                                Some(string) => string,
                                None => "",
                            };
                            msg = msg.replace(&value_model, &("\"".to_owned() + &string + "\""));
                        } else if parsed[&label].is_number() {
                            let num = match parsed[&label].as_number() {
                                Some(num) => num,
                                None => {
                                    warn!("parse JSON number failed");
                                    return Err(());
                                },
                            };
                            msg = msg.replace(&value_model, &num.to_string());
                        } else {
                            warn!("unsurported data type");
                        }
                    },
                    Err(err) => {
                        warn!("get label from model {:#?} failed: {:#?}", model, err);
                    },
                }
                
            }
        },
        Err(_err) => {},
    }
    match template.get_call_models() {
        Ok(call_models) => {
            for model in call_models {
                match model.get_call_result() {
                    Ok(value) => {
                        let call_model = match model {
                            Model::Value(call_model) => call_model,
                        };
                        match value {
                            Value::Number(num) => {
                                msg = msg.replace(&call_model, &num.to_string());
                            },
                            Value::String(string) => {
                                msg = msg.replace(&call_model, &string);
                            },
                        }
                    },
                    Err(err) => {
                        error!("get call result failed: {:#?}", err);
                    },
                }
            }
        },
        Err(_err) => {},
    }
    if let Err(_err) = json::parse(&msg) {
        error!("msg converted was not a JSON string: {}", msg);
        return Err(());
    }
    Ok(msg)
}

#[cfg(feature = "data_interface_serial_port")]
fn init_data_interface(if_name: &str, if_type: &str) -> Result<serial::SystemPort, DataIfError> {
    if if_type.eq("serial_port") {
        const SETTINGS: serial::PortSettings = serial::PortSettings {
            baud_rate:    serial::Baud115200,
            char_size:    serial::Bits8,
            parity:       serial::ParityNone,
            stop_bits:    serial::Stop1,
            flow_control: serial::FlowNone,
        };
        let mut port = match serial::open(&if_name) {
            Ok(port) => port,
            Err(err) => {
                error!("Open {} failed: {}", if_name, err);
                return Err(DataIfError::DataIfOpenError);
            },
        };
        if let Err(err) = port.configure(&SETTINGS) {
            error!("serial port config failed: {}", err);
            return Err(DataIfError::DataIfInitError);
        };
        if let Err(err) = port.set_timeout(Duration::from_secs(5)) {
            error!("serial port config failed: {}", err);
            return Err(DataIfError::DataIfInitError);
        };
        return Ok(port);
    } else {
        return Err(DataIfError::DataIfUnknownType);
    }
}

#[cfg(feature = "data_interface_text_file")]
fn init_data_interface(if_name: &str, if_type: &str) -> Result<FileIf, DataIfError> {
    if if_type.eq("text_file") {
        match File::open(if_name) {
            Ok(_file) => return Ok(FileIf),
            Err(_err) => return Err(DataIfError::DataIfOpenError),
        }
    } else {
        return Err(DataIfError::DataIfUnknownType);
    }
}

fn main() {
    env::set_var("RUST_LOG", env::var_os("RUST_LOG").unwrap_or_else(|| "info".into()));
    env_logger::init();

    let matches = App::new("pepper_gateway")
        .author("Yu Yu <qianchenzhumeng@live.cn>")
        .arg(
            Arg::with_name("CONFIG_FILE")
                .short("c")
                .long("config-file")
                .takes_value(true)
                .required(true)
                .help("specify the broker config file."),
        ).get_matches();

    let config_file = matches.value_of("CONFIG_FILE").unwrap();
    let toml_string = fs::read_to_string(&config_file).unwrap();
    let config: Config = toml::from_str(&toml_string).unwrap();
    let server_addr = config.server.address;
    let client_id = config.client.id;
    let pub_topic = config.topic.pub_topic;
    let channel_filters: Vec<(TopicFilter, QualityOfService)> = vec![(TopicFilter::new(config.topic.sub_topic).unwrap(), QualityOfService::Level0)];
    let database_path = config.database.path;
    let database_name = config.database.name;
    let template = config.msg.template;
    let msg_example = config.msg.example;
    let data_if_name = config.data_if.if_name;
    let data_if_type = config.data_if.if_type;
    let mut try_to_connect = true;

    // 数据模板校验
    let check = match format_msg(&msg_example, &template) {
        Ok(string) => string,
        Err(err) => panic!("please check msg.example and msg.template if config-file: {:#?}", err),
    };
    if let Err(_) = json::parse(&check) {
        panic!("please check msg.example and msg.template config-file: {}", config_file);
    }

    let mut data_if = match init_data_interface(&data_if_name, &data_if_type) {
        Ok(data_if) => {
            data_if
        },
        Err(err) => {
            panic!("Init data interface failed: {:#?}", err)
        },
    };

    let db = match init_data_base(&database_path, &database_name) {
        Ok(database) => database,
        Err(err) => {
            panic!("Problem opening the database: {:?}", err)
        },
    };

    let (insert_req, db_handle) = mpsc::channel();
    let query_req =  mpsc::Sender::clone(&insert_req);
    let (db_query_rep_tx, db_query_rep_rx) = mpsc::channel();
    let db_delete_req_tx =  mpsc::Sender::clone(&insert_req);
    let (original_data_pub_strem_tx, original_data_pub_strem_rx) = mpsc::channel();
    let (offine_data_pub_stream_tx, offine_data_pub_stream_rx) = mpsc::channel();
    let (db_delete_rep_tx, db_delete_rep_rx) = mpsc::channel();

    let original_data_pub_topic = pub_topic.clone();
    let original_data_pub_template = template.clone();
    let (original_data_tx, original_data_rx) = mpsc::channel();

    // 获取原始数据
    #[cfg(feature = "data_interface_serial_port")]
    thread::spawn(move || {
        let mut hdtp = Hdtp::new();
        let mut buf: Vec<u8> = vec![0];
        loop {
            match data_if.read(&mut buf[..]) {
                Ok(_n) => {
                    //println!("{}", hdtp);
                    hdtp.input(buf[0]);
                    match hdtp.get_msg() {
                        Ok(msg) => {
                            match original_data_tx.send(msg) {
                                _ => {},
                            }
                        },
                        Err(_) => {},
                    }
                },
                Err(_err) => continue,
            }
        }
    });

    #[cfg(feature = "data_interface_text_file")]
    thread::spawn(move || {
        loop {
            match data_if.read(&data_if_name) {
                Ok(sn_msg) => {
                    if !sn_msg.is_empty() {
                        match original_data_tx.send(sn_msg) {
                            _ => {},
                        }
                    }
                },
                Err(_err) => continue,
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    thread::spawn(move || {
        let mut original_data_pub_strem_option: Option<TcpStream> = None;
        let mut network_ok = false;
        let mut published = false;
        loop {
            match original_data_rx.recv() {
                Ok(sn_msg) => {
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
                                    published = false;
                                },
                            }
                        },
                        Err(_err) => {},
                    }
                    if network_ok {
                            match original_data_pub_strem_option {
                                Some(ref mut stream) => {
                                    match format_msg(&sn_msg, &original_data_pub_template) {
                                        Ok(pub_msg) => {
                                            let r = pub_sn_msg_use_stream(&original_data_pub_topic, &pub_msg, stream);
                                            match r {
                                                Ok(_ok) => {
                                                        published = true;
                                                        info!("pub msg successfully: {}", &pub_msg);
                                                    },
                                                Err(err) => error!("pub msg failed: {:#?}", err),
                                            }
                                        },
                                        Err(err) => {
                                            error!("convert from data template failed: {:#?}", err);
                                        },
                                    };
                                    
                                },
                                None => {},
                            }
                    }
                    if !published {
                            match get_data_from_msg(&sn_msg) {
                                Ok(device_data) => {
                                    let db_req = DbReq{
                                        operation: DbOp::INSERT,
                                        id: 0,
                                        data: device_data,
                                    };
                                    match insert_req.send(db_req) {
                                        Err(err) => {
                                            error!("send insert req failed: {}", err);
                                        },
                                        _ => {},
                                    }
                                },
                                Err(_err) => {
                                    error!("get data from {} failed", &sn_msg);
                                },
                            };
                    }
                },
                Err(_err) => {},
            }
        }
    });

    let offine_data_pub_topic = pub_topic.clone();
    let offline_data_pub_template = template.clone();
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
                data: DeviceData::new(0, "", 0, "", 0.0, 0.0, 0.0, 0, 0),
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
                                let sn_msg = get_msg_from_data(&device_data);
                                let pub_msg = match format_msg(&sn_msg, &offline_data_pub_template) {
                                    Ok(pub_msg) => pub_msg,
                                    Err(err) => {
                                        error!("convert from data template failed: {:#?}", err);
                                        continue;
                                    },
                                };
                                match offine_data_pub_stream_option {
                                            Some(ref mut stream) => {
                                                if let Ok(_) = pub_sn_msg_use_stream(&offine_data_pub_topic, &pub_msg, stream) {
                                                    //数据上传成功，删除数据库中对应的记录
                                                    let db_delete_req = DbReq{
                                                        operation: DbOp::DELETE,
                                                        id: id,
                                                        data: DeviceData::new(0, "", 0, "", 0.0, 0.0, 0.0, 0, 0),
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
                            name: row.get(2)?,
                            timestamp_msec: row.get(3)?,
                            time_string: row.get(4)?,
                            temperature: row.get(5)?,
                            humidity: row.get(6)?,
                            voltage: row.get(7)?,
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
        match offine_data_pub_stream_tx.send(msg) {
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
