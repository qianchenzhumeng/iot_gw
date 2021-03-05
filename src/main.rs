extern crate chrono;
extern crate clap;
extern crate data_management;
extern crate data_template;
#[cfg(feature = "data_interface_serial_port")]
extern crate hdtp;
extern crate json;
extern crate log;
extern crate log4rs;
extern crate rusqlite;
#[cfg(feature = "data_interface_text_file")]
extern crate sensor_interface;
extern crate serde_derive;
#[cfg(feature = "data_interface_serial_port")]
extern crate serial;
extern crate time;
extern crate toml;
extern crate uuid;

use chrono::{Local, DateTime};
use data_management::data_management::{data_base, DeviceData};
use std::time::Duration;

use clap::{App, Arg};
use data_template::{Model, Template, Value};
#[cfg(feature = "data_interface_serial_port")]
use hdtp::Hdtp;
use serde_derive::Deserialize;
#[cfg(feature = "data_interface_serial_port")]
use serial::prelude::*;
use std::io::prelude::*;
use std::sync::mpsc;
use std::{env, fs, process, str, thread};
#[cfg(feature = "ssl")]
use std::path::Path;
extern crate paho_mqtt as mqtt;

#[cfg(feature = "data_interface_text_file")]
use sensor_interface::FileIf;
#[cfg(feature = "data_interface_text_file")]
use std::fs::File;

use log::{error, warn, info, debug, LevelFilter};
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        rolling_file::{
            RollingFileAppender,
            policy::compound::{
                CompoundPolicy,
                roll::fixed_window::FixedWindowRoller,
                trigger::size::SizeTrigger,
            },
        },
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};

#[derive(Debug)]
enum DataIfError {
    DataIfOpenError,
    #[cfg(feature = "data_interface_serial_port")]
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

#[derive(Debug)]
enum DatumType {
    Message,
    Notice,
}

#[derive(Debug)]
struct Datum {
    id: u32,
    datum_type: DatumType,
    value: DeviceData,
}

#[derive(Deserialize)]
struct AppConfig {
    log: LogConfig,
    server: ServerConfig,
    #[cfg(feature = "ssl")]
    tls: TlsFiles,
    client: ClientConfig,
    topic: TopicConfig,
    msg: MsgConfig,
    database: DatabaseConfig,
    data_if: DataIfConfig,
}

#[derive(Deserialize)]
struct LogConfig {
    file_path: String,
    file_path_pattern: String,
    level: String,
    count: u32,
    size: u64,
}

#[derive(Deserialize)]
struct ServerConfig {
    address: String,
}

#[cfg(feature = "ssl")]
#[derive(Deserialize)]
struct TlsFiles {
    cafile: String,
    key_store: String,
}

#[derive(Deserialize)]
struct ClientConfig {
    id: String,
    keep_alive: u16,
    username: String,
}

#[derive(Deserialize)]
struct TopicConfig {
    sub_topic: String,
    pub_topic: String,
    pub_log_topic: String,
    qos: i32,
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

fn init_app_log(log_config: &LogConfig) -> Result<(), ()> {
    let level = match &log_config.level[..] {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    // Build a stdout logger.
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)} {l} {M}:{T}] {m}\n",
        )))
        .target(Target::Stdout)
        .build();

    // Logging to log file.
    let trigger = SizeTrigger::new(log_config.size);
    let roller = match FixedWindowRoller::builder()
        .build(&log_config.file_path_pattern, log_config.count){
            Ok(roller) => roller,
            Err(_) => return Err(()),
        };
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));
    let logfile = match RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)} {l} {M}:{T}] {m}\n",
        )))
        .append(true)
        .build(&log_config.file_path, Box::new(policy))
    {
        Ok(logfile) => logfile,
        Err(_) => return Err(()),
    };

    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stdout.
    let config = match Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(
            Root::builder()
                .appender("logfile")
                .appender("stdout")
                .build(level),
        ) {
        Ok(config) => config,
        Err(_) => return Err(()),
    };

    if let Err(_) = log4rs::init_config(config) {
        return Err(());
    }
    Ok(())
}

fn init_data_base(path: &str, name: &str) -> Result<rusqlite::Connection, ()> {
    let db = data_base::open_data_base(path, name);

    let db = match db {
        Ok(database) => database,
        Err(err) => panic!("Problem opening the database: {:?}", err),
    };

    if data_base::device_data_table_exsits(&db) {
        return Ok(db);
    }

    match data_base::create_device_data_table(&db) {
        Ok(_ok) => debug!("create DEVICE_DATA table successfully"),
        Err(err) => error!("create DEVICE_DATA table failed: {:?}", err),
    }

    Ok(db)
}

fn get_msg_from_data(data: &DeviceData) -> String {
    data.msg.clone()
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
                                }
                            };
                            msg = msg.replace(&value_model, &num.to_string());
                        } else {
                            warn!("unsurported data type");
                        }
                    }
                    Err(err) => {
                        warn!("get label from model {:#?} failed: {:#?}", model, err);
                    }
                }
            }
        }
        Err(_err) => {}
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
                            }
                            Value::String(string) => {
                                msg = msg.replace(&call_model, &string);
                            }
                        }
                    }
                    Err(err) => {
                        error!("get call result failed: {:#?}", err);
                    }
                }
            }
        }
        Err(_err) => {}
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
            baud_rate: serial::Baud115200,
            char_size: serial::Bits8,
            parity: serial::ParityNone,
            stop_bits: serial::Stop1,
            flow_control: serial::FlowNone,
        };
        let mut port = match serial::open(&if_name) {
            Ok(port) => port,
            Err(err) => {
                error!("Open {} failed: {}", if_name, err);
                return Err(DataIfError::DataIfOpenError);
            }
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

// 格式化 log 信息
fn format_log(msg: &str) -> Result<String, ()> {
    let local: DateTime<Local> = Local::now(); // 本地时间
    let time_str = local.format("%Y-%m-%d %H:%M:%S").to_string();
    let log: String;
    if msg.is_empty() {
        log = format!("{{\"LOGS\": \"[{}]\"}}", time_str);
    } else {
        log = format!("{{\"LOGS\": \"[{}]: {}\"}}", time_str, msg);
    }
    info!("{}", log);
    Ok(log)
}

fn main() {
    env::set_var(
        "RUST_LOG",
        env::var_os("RUST_LOG").unwrap_or_else(|| "info".into()),
    );

    let matches = App::new("pepper_gateway")
        .author("Yu Yu <qianchenzhumeng@live.cn>")
        .arg(
            Arg::with_name("CONFIG_FILE")
                .short("c")
                .long("config-file")
                .takes_value(true)
                .required(true)
                .help("specify the broker config file."),
        )
        .get_matches();

    let config_file = matches.value_of("CONFIG_FILE").unwrap();
    let toml_string = fs::read_to_string(&config_file).unwrap();
    let config: AppConfig = toml::from_str(&toml_string).unwrap();
    let app_log = config.log;
    let server_addr = config.server.address;
    #[cfg(feature = "ssl")]
    let tls = config.tls;
    let client = config.client;
    let keep_alive = client.keep_alive;
    let pub_topic = config.topic.pub_topic;
    let pub_log_topic = config.topic.pub_log_topic;
    let sub_topic = config.topic.sub_topic;
    let qos = config.topic.qos;
    let database_path = config.database.path;
    let database_name = config.database.name;
    let template = config.msg.template;
    let msg_example = config.msg.example;
    let data_if_name = config.data_if.if_name;
    let data_if_type = config.data_if.if_type;

    init_app_log(&app_log).unwrap();

    // 数据模板校验
    let check = match format_msg(&msg_example, &template) {
        Ok(string) => string,
        Err(err) => panic!(
            "please check msg.example and msg.template if config-file: {:#?}",
            err
        ),
    };
    if let Err(_) = json::parse(&check) {
        panic!(
            "please check msg.example and msg.template config-file: {}",
            config_file
        );
    }

    // 检查证书和密钥库是否存在
    #[cfg(feature = "ssl")]
    if !Path::new(&tls.cafile).exists() {
        panic!(
            "The trust store file does not exist: {}", &tls.cafile
        );
    } else {
        if !Path::new(&tls.key_store).exists() {
            panic!(
                "The key store file does not exist: {}", &tls.key_store
            );
        }
    }

    #[cfg(feature = "data_interface_serial_port")]
    let mut data_if = match init_data_interface(&data_if_name, &data_if_type) {
        Ok(data_if) => data_if,
        Err(err) => panic!("Init data interface failed: {:#?}", err),
    };
    #[cfg(feature = "data_interface_text_file")]
    let data_if = match init_data_interface(&data_if_name, &data_if_type) {
        Ok(data_if) => data_if,
        Err(err) => panic!("Init data interface failed: {:#?}", err),
    };

    let db = match init_data_base(&database_path, &database_name) {
        Ok(database) => database,
        Err(err) => panic!("Problem opening the database: {:?}", err),
    };

    let (insert_req, db_handle) = mpsc::channel();
    let query_req = mpsc::Sender::clone(&insert_req);
    let (db_query_rep_tx, db_query_rep_rx) = mpsc::channel();
    let db_delete_req_tx = mpsc::Sender::clone(&insert_req);
    let (db_delete_rep_tx, db_delete_rep_rx) = mpsc::channel();

    let original_data_pub_template = template.clone();

    let (original_data_tx, original_data_rx) = mpsc::channel();
    // 获取原始数据
    let original_data_read_thread_builder =
        thread::Builder::new().name("original_data_read_thread".into());
    #[cfg(feature = "data_interface_serial_port")]
    let original_data_read_thread = original_data_read_thread_builder
        .spawn(move || {
            let mut hdtp = Hdtp::new();
            let mut buf: Vec<u8> = vec![0];
            loop {
                match data_if.read(&mut buf[..]) {
                    Ok(_n) => {
                        hdtp.input(buf[0]);
                        match hdtp.get_msg() {
                            Ok(msg) => match original_data_tx.send(msg) {
                                _ => {}
                            },
                            Err(_) => {}
                        }
                    }
                    Err(_err) => continue,
                }
            }
        })
        .unwrap();

    #[cfg(feature = "data_interface_text_file")]
    let original_data_read_thread = original_data_read_thread_builder
        .spawn(move || loop {
            match data_if.read(&data_if_name) {
                Ok(sn_msg) => {
                    if !sn_msg.is_empty() {
                        match original_data_tx.send(String::from(sn_msg.trim())) {
                            _ => {}
                        }
                    }
                }
                Err(_err) => continue,
            }
            thread::sleep(Duration::from_secs(1));
        })
        .unwrap();

    let (original_datum_sender, datum_receiver) = mpsc::channel();
    let buffed_datum_sender = original_datum_sender.clone();
    let (datum_publish_sender, datum_publish_receiver) = mpsc::channel();

    let (publish_result_sender, publish_result_receiver) = mpsc::channel();

    let (cloud_statue_announcement_sender, cloud_statue_announcement_receiver) = mpsc::channel();
    let cloud_statue_announcement_sender_clone = cloud_statue_announcement_sender.clone();
    let send_cloud_link_broken_msg_to_uploader = datum_publish_sender.clone();
    let (cloud_link_broken_msg_sender, cloud_link_change_msg_receiver) = mpsc::channel();
    let send_cloud_link_change_msg_to_data_manager = original_datum_sender.clone();

    // 原始数据处理
    let original_data_handle_thread_builder =
        thread::Builder::new().name("original_data_handle_thread".into());
    let original_data_handle_thread = original_data_handle_thread_builder
        .spawn(move || loop {
            match original_data_rx.recv() {
                Ok(sn_msg) => {
                    match format_msg(&sn_msg, &original_data_pub_template) {
                        Ok(formated_msg) => {
                            match buffed_datum_sender.send(Datum {
                                id: 0,
                                datum_type: DatumType::Message,
                                value: DeviceData::new(&formated_msg),
                            }) {
                                Err(err) => error!("send datum to data_manger failed: {}", err),
                                _ => {}
                            }
                        }
                        Err(_) => {
                            error!("convert from data template failed: {}", sn_msg);
                            continue;
                        }
                    };
                }
                Err(_err) => {}
            }
        })
        .unwrap();

    // 离线数据处理
    let offine_data_handle_thread_builder =
        thread::Builder::new().name("offine_data_handle_thread".into());
    let offine_data_handle_thread = offine_data_handle_thread_builder
        .spawn(move || {
            for msg in cloud_link_change_msg_receiver.iter() {
                if let Some(_msg) = msg {
                    /* 收到联网消息 */
                    let db_req = DbReq {
                        operation: DbOp::QUERY,
                        id: 0,
                        data: DeviceData::new(""),
                    };
                    match query_req.send(db_req) {
                        Ok(_ok) => match db_query_rep_rx.recv() {
                            Ok(vec) => {
                                for tuple in vec {
                                    let (id, device_data) = match tuple {
                                        Ok(t) => t,
                                        Err(err) => {
                                            error!("get data from data iter failed: {}", err);
                                            continue;
                                        }
                                    };
                                    let sn_msg = get_msg_from_data(&device_data);
                                    match original_datum_sender.send(Datum {
                                        id: id,
                                        datum_type: DatumType::Message,
                                        value: DeviceData::new(&sn_msg),
                                    }) {
                                        Err(err) => {
                                            error!("send datum to data_manger failed: {}", err)
                                        }
                                        _ => {}
                                    }
                                    thread::sleep(Duration::from_millis(100));
                                }
                            }
                            Err(_err) => {}
                        },
                        Err(_err) => {}
                    }
                }
            }
        })
        .unwrap();

    // 数据库操作
    let db_handle_thread_builder = thread::Builder::new().name("db_handle_thread".into());
    let db_handle_thread = db_handle_thread_builder
        .spawn(move || {
            loop {
                let db_req = match db_handle.recv() {
                    Ok(req) => req,
                    Err(_err) => continue,
                };
                match db_req.operation {
                    DbOp::INSERT => {
                        //let sn_msg = get_msg_from_data(&db_req.data);
                        //let device_data = match format_msg(&sn_msg, &db_handle_template) {
                        //    Ok(msg) => {
                        //        DeviceData::new(&msg)
                        //    },
                        //    Err(err) => {
                        //        error!("convert from data template failed: {:#?}", err);
                        //        continue;
                        //    },
                        //};
                        match data_base::insert_data_to_device_data_table(&db, &db_req.data) {
                            Ok(_ok) => {
                                debug!("buffed data successfully");
                            }
                            Err(err) => {
                                error!("buffed data  failed: {:?}", err);
                            }
                        }
                    }
                    DbOp::QUERY => {
                        let mut stmt = match data_base::querry_device_data(&db) {
                            Ok(stmt) => stmt,
                            Err(_err) => {
                                error!("querry database failed");
                                continue;
                            }
                        };
                        let data_iter = match stmt.query_map(rusqlite::params![], |row| {
                            let id: u32 = row.get(0)?;
                            let data = DeviceData { msg: row.get(1)? };
                            Ok((id, data))
                        }) {
                            Ok(iter) => iter,
                            Err(_err) => {
                                error!("get data iter failed");
                                continue;
                            }
                        };
                        let mut vec = Vec::new();
                        for tuple in data_iter {
                            vec.push(tuple);
                        }
                        match db_query_rep_tx.send(vec) {
                            _ => {}
                        }
                    }
                    DbOp::DELETE => match data_base::delete_device_data(&db, db_req.id) {
                        Ok(_ok) => match db_delete_rep_tx.send(true) {
                            Ok(_ok) => {}
                            Err(err) => error!("send delete rep failed: {}", err),
                        },
                        Err(_err) => match db_delete_rep_tx.send(false) {
                            Ok(_ok) => {}
                            Err(err) => error!("send delete rep failed: {}", err),
                        },
                    },
                }
            }
        })
        .unwrap();

    // 数据管理
    let data_manager_builder = thread::Builder::new().name("data_manager".into());
    let data_manager = data_manager_builder.spawn(move || {
        let mut id: u32;
        let mut cloud_is_connected = false;
        for datum in datum_receiver.iter() {
            info!("datum: {{ id: {}, type: {:?}, value: ... }}", datum.id, datum.datum_type);
            match datum.datum_type {
                DatumType::Notice => {
                    match datum.id {
                        0 => cloud_is_connected = false,    /* 网络断开 */
                        _ => cloud_is_connected = true,    /* 网络已连接 */
                    }
                    continue;
                },
                DatumType::Message => {},
            }
            id = datum.id;
            if cloud_is_connected { /* 已联网，发布数据 */
                match datum_publish_sender.send(Some(datum.value.msg.clone())) {
                    Ok(_) => {},
                    Err(err) => error!("Error send datum to publish: {}", err),
                }
                match publish_result_receiver.recv() {
                    Ok(r) => {
                        if r == false {
                            if id == 0 {
                                // 原始数据 id 为 0，发布失败，需要存入数据库
                                let db_req = DbReq{
                                    operation: DbOp::INSERT,
                                    id: 0,
                                    data: datum.value,
                                };
                                match insert_req.send(db_req) {
                                    Err(err) => {
                                        error!("send insert req failed: {}", err);
                                    },
                                    _ => {},
                                }
                            }
                            // 离线数据原本就在数据库中，发布失败后不做处理
                        } else {
                            if id != 0 {
                                let db_delete_req = DbReq{
                                    operation: DbOp::DELETE,
                                    id: id,
                                    data: DeviceData::new(""),
                                };
                                match db_delete_req_tx.send(db_delete_req) {
                                    Ok(_ok) => {
                                        match db_delete_rep_rx.recv() {
                                            Ok(r) => {
                                                if r {
                                                    debug!("handle offline data successfully");
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
                            // 原始数据发布成功后不做处理
                        }
                    },
                    Err(err) => error!("Error receiving publish result: {}", err),
                }
            } else if id == 0 {    /* 没联网，存入数据库 */
                let db_req = DbReq{
                    operation: DbOp::INSERT,
                    id: 0,
                    data: datum.value,
                };
                match insert_req.send(db_req) {
                    Err(err) => {
                        error!("send insert req failed: {}", err);
                    },
                    _ => {},
                }
            }
        }
    }).unwrap();

    let cloud_state_broadcaster_builder =
        thread::Builder::new().name("cloud_state_broadcaster".into());
    let cloud_state_broadcaster = cloud_state_broadcaster_builder
        .spawn(move || {
            for msg in cloud_statue_announcement_receiver.iter() {
                if let Some(_msg) = msg {
                    info!("Successfully connected.");
                    // 发送联网消息给离线数据处理线程
                    if let Err(err) = cloud_link_broken_msg_sender.send(Some(0)) {
                        error!("Error send cloud link broken msg: {}", err);
                    }
                    // 发送联网消息给数据管理模块
                    if let Err(err) = send_cloud_link_change_msg_to_data_manager.send(Datum {
                        id: 1,
                        datum_type: DatumType::Notice,
                        value: DeviceData::new(""),
                    }) {
                        error!("Error send cloud link change msg to data manager: {}", err);
                    }
                } else {
                    error!("Cloud connection lost.");
                    // 发送断网消息给数据发布线程
                    if let Err(err) = send_cloud_link_broken_msg_to_uploader.send(None) {
                        error!("Error send cloud link broken msg to uploader: {}", err);
                    }
                    // 发送断网消息给数据管理模块
                    if let Err(err) = send_cloud_link_change_msg_to_data_manager.send(Datum {
                        id: 0,
                        datum_type: DatumType::Notice,
                        value: DeviceData::new(""),
                    }) {
                        error!("Error send cloud link change msg to data manager: {}", err);
                    }
                }
            }
        })
        .unwrap();

    // 处理 MQTT 连接
    type MsgReceiver = mpsc::Receiver<Option<mqtt::Message>>;
    let (tx, rx): (mpsc::Sender<MsgReceiver>, mpsc::Receiver<MsgReceiver>) = mpsc::channel();
    let mqtt_pub_thread_builder = thread::Builder::new().name("mqtt_pub_thread".into());
    let mqtt_pub_thread = mqtt_pub_thread_builder
        .spawn(move || {
            let create_opts = mqtt::CreateOptionsBuilder::new()
                .server_uri(server_addr)
                .client_id(client.id)
                .max_buffered_messages(1) // 离线时不缓存数据
                .finalize();

            let mut cli = mqtt::Client::new(create_opts).unwrap_or_else(|e| {
                error!("Error creating the client: {:?}", e);
                process::exit(1);
            });

            cli.set_timeout(Duration::from_secs(5));
            let sub_msg_receiver = cli.start_consuming();

            #[cfg(feature = "ssl")]
            let ssl_opts = mqtt::SslOptionsBuilder::new()
                .trust_store(&tls.cafile)
                .key_store(&tls.key_store)
                .finalize();

            #[cfg(feature = "ssl")]
            let conn_opts = mqtt::ConnectOptionsBuilder::new()
                .ssl_options(ssl_opts)
                .keep_alive_interval(Duration::from_secs(keep_alive.into()))
                .mqtt_version(mqtt::MQTT_VERSION_3_1_1)
                .clean_session(true)
                .user_name(client.username)
                .finalize();

            #[cfg(not(feature = "ssl"))]
            let conn_opts = mqtt::ConnectOptionsBuilder::new()
                .keep_alive_interval(Duration::from_secs(keep_alive.into()))
                .mqtt_version(mqtt::MQTT_VERSION_3_1_1)
                .clean_session(true)
                .user_name(client.username)
                .finalize();

            info!("Connecting to the MQTT broker...");
            match cli.connect(conn_opts) {
                Ok(rsp) => {
                    if let Some((server_uri, ver, session_present)) = rsp.connect_response() {
                        if let Err(err) = cloud_statue_announcement_sender_clone.send(Some(0)) {
                            error!("Error send cloud statue announcement: {}", err);
                        }
                        info!("Connected to: '{}' with MQTT version {}", server_uri, ver);
                        if !session_present {
                            // Register subscriptions on the server
                            debug!("Subscribing to topics, with requested QoS: {:?}...", qos);

                            match cli.subscribe(&sub_topic, qos) {
                                Ok(qosv) => debug!("QoS granted: {:?}", qosv),
                                Err(e) => {
                                    debug!("Error subscribing to topics: {:?}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error connecting to the broker: {:?}", e);
                    loop {
                        if cli.reconnect().is_ok() {
                            if let Err(err) = cloud_statue_announcement_sender_clone.send(Some(0)) {
                                error!("Error send cloud statue announcement: {}", err);
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
                match datum_publish_receiver.recv_timeout(Duration::from_secs(10)) {
                Ok(option) => if let Some(msg) = option {
                    let message = mqtt::Message::new(pub_topic.clone(), msg, qos);
                    debug!("message: {}", message);
                    if let Err(e) = cli.publish(message) {
                        error!("Error publishing message: {:?}", e);
                        publish_result = false;
                    } else {
                        publish_result = true;
                        // 数据发布成功后发送 LOG
                        match format_log("") {
                            Ok(log) => {
                                let log_msg = mqtt::Message::new(pub_log_topic.clone(), log, qos);
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
                if !cli.is_connected() {
                    if cli.reconnect().is_ok() {
                        if let Err(err) = cloud_statue_announcement_sender_clone.send(Some(0)) {
                            error!("Error send cloud statue announcement: {}", err);
                        }
                    }
                }
            }
        })
        .unwrap();

    let mqtt_sub_thread_builder = thread::Builder::new().name("mqtt_sub_thread".into());
    let mqtt_sub_thread = mqtt_sub_thread_builder
        .spawn(move || loop {
            match rx.recv() {
                Ok(r) => {
                    for msg in r.iter() {
                        if let Some(msg) = msg {
                            debug!("{}", msg);
                        } else {
                            if let Err(err) = cloud_statue_announcement_sender.send(None) {
                                error!("Error send cloud statue announcement: {}", err);
                            }
                        }
                    }
                }
                Err(err) => {
                    error!("mqtt_sub_thread recv error: {}", err);
                    break;
                }
            }
        })
        .unwrap();

    original_data_read_thread.join().unwrap();
    original_data_handle_thread.join().unwrap();
    offine_data_handle_thread.join().unwrap();
    db_handle_thread.join().unwrap();
    data_manager.join().unwrap();
    cloud_state_broadcaster.join().unwrap();
    mqtt_pub_thread.join().unwrap();
    mqtt_sub_thread.join().unwrap();
}
