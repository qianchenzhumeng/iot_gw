extern crate chrono;
extern crate clap;
extern crate data_template;
extern crate json;
extern crate log;
extern crate log4rs;
extern crate rusqlite;
extern crate serde_derive;
extern crate time;
extern crate toml;
extern crate uuid;
extern crate shadow_rs;
extern crate min_rs as min;
mod mqtt;
mod types;
mod interface;
mod data_manager;

use types::{ClientConfig, TopicConfig, TlsFiles, MsgReceiver};

use chrono::{Local, DateTime};
use data_manager::data_management::{data_base, DeviceData};
use std::time::Duration;
use shadow_rs::shadow;
use clap::{App, Arg, crate_name, crate_version, crate_authors};
use data_template::{Model, Template, Value};
use serde_derive::Deserialize;
use serialport::SerialPort;
use std::io::prelude::*;
use std::sync::mpsc;
use std::{env, fs, str, thread};
#[cfg(feature = "ssl")]
use std::path::Path;
use interface::{FileIf, HwIf, SpiIf};
use std::fs::File;
use spidev::{SpiModeFlags, Spidev, SpidevOptions};
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

shadow!(build);

struct SensorInterface {
    text_file: String,
    serial_port: Option<Box<dyn SerialPort>>,
    spi: Option<Spidev>,
}

#[derive(Debug)]
enum DataIfError {
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

fn init_data_interface(if_name: &str, if_type: &str) -> Result<SensorInterface, DataIfError> {
    if if_type.eq("serial_port") {
        let port = match serialport::new(if_name, 115200)
            .timeout(Duration::from_millis(10))
            .open()
        {
            Ok(port) => port,
            Err(err) => {
                error!("Open {} failed: {}", if_name, err);
                return Err(DataIfError::DataIfOpenError);
            }
        };
        return Ok(SensorInterface{ text_file: if_name.to_string(), serial_port: Some(port), spi: None});
    } else if if_type.eq("text_file") {
        match File::open(if_name) {
            Ok(_file) => return Ok(SensorInterface{ text_file: if_name.to_string(), serial_port: None, spi: None}),
            Err(_err) => return Err(DataIfError::DataIfOpenError),
        }
    } else if if_type.eq("spi_sx1276") {
        let spi = match Spidev::open(if_name) {
            Ok(mut spi) => {
                let options = SpidevOptions::new()
                    .bits_per_word(8)
                    .max_speed_hz(1000_000)
                    .mode(SpiModeFlags::SPI_MODE_0)
                    .build();
                match spi.configure(&options) {
                    Ok(_) => {
                        if let Ok(_) = SpiIf.init(&mut spi) {
                            spi
                        } else {
                            error!("init spi device failed: {}", if_name);
                            return Err(DataIfError::DataIfOpenError);
                        }
                    }
                    Err(err) => {
                        error!("Open {} failed: {}", if_name, err);
                        return Err(DataIfError::DataIfOpenError);
                    }
                }
            }
            Err(err) => {
                error!("Open {} failed: {}", if_name, err);
                return Err(DataIfError::DataIfOpenError);
            }
        };
        return Ok(SensorInterface {
            text_file: if_name.to_string(),
            serial_port: None,
            spi: Some(spi),
        });
    } else {
        error!("DataIf type unknown: {}", if_type);
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

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .long_version(build::version().as_str())
        .author(crate_authors!())
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
    #[cfg(not(feature = "ssl"))]
    let tls = TlsFiles{cafile: String::from(""), key_store: String::from("")};  // 如果没有启用 tsl，生成个空的。
    let client = config.client;
    let topic = config.topic;
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

    let sensor_if = match init_data_interface(&data_if_name, &data_if_type) {
        Ok(sensor_if) => sensor_if,
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

    // 下行消息收发
    let (downstream_msg_tx, downstream_msg_rx): (mpsc::Sender<String>, mpsc::Receiver<String>) = mpsc::channel();

    // 获取原始数据
    let original_data_read_thread_builder = thread::Builder::new().name("original_data_read_thread".into());

    let original_data_read_thread = original_data_read_thread_builder
        .spawn(move || {
            let mut buf: Vec<u8> = (0..255).collect();
            if data_if_type.eq("serial_port") {
                if let Some(port) = sensor_if.serial_port {
                    let uart = HwIf::new(port, String::from("uart"), 128);
                    let mut min = min::Context::new(
                        String::from("min"),
                        &uart,
                        0,
                        false,
                    );
                    loop {
                        if let Ok(msg) = downstream_msg_rx.try_recv() {
                            info!("{}", msg);
                            if let Err(_) = min.send_frame(0, msg.as_bytes(), msg.len() as u8)
                            {
                                error!("Send msg to interface failed.");
                            }
                        }
                        if let Ok(n) = min.hw_if.read(&mut buf[..]) {
                            min.poll(&buf[0..n], n as u32);
                        };
                        if let Ok(msg) = min.get_msg() {
                            if let Ok(string) = String::from_utf8(msg.buf[0..msg.len as usize].to_vec()) {
                                match original_data_tx.send(string) {
                                    _ => {}
                                }
                            }
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                };
            } else if data_if_type.eq("spi_sx1276") {
                if let Some(mut spi) = sensor_if.spi {
                    loop {
                        if let Ok(msg) = downstream_msg_rx.try_recv() {
                            info!("{}", msg);
                        }
                        match SpiIf.read(&mut spi) {
                            Ok(sn_msg) => {
                                if !sn_msg.is_empty() {
                                    match original_data_tx.send(String::from(sn_msg.trim())) {
                                        _ => {}
                                    }
                                }
                            }
                            Err(_err) => continue,
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            } else {
                loop {
                    if let Ok(msg) = downstream_msg_rx.try_recv() {
                        info!("{}", msg);
                    }
                    match FileIf.read(&sensor_if.text_file) {
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
                }
            }
        })
        .unwrap();

    // 该通道既收发数据，又收发消息。如果是收发消息，id 为 0 表示网络断开，id 为 1 表示网络已连接。
    let (original_datum_sender, datum_receiver) = mpsc::channel();
    let buffed_datum_sender = original_datum_sender.clone();

    // 该通道用于将数据发送给数据上传线程
    let (datum_publish_sender, datum_publish_receiver): (mpsc::Sender<Option<String>>, mpsc::Receiver<Option<String>>) = mpsc::channel();
    // 该通道用于向数据发送者返回数据上传结果
    let (publish_result_sender, publish_result_receiver) = mpsc::channel();

    // 该通道用于将封装好的 MQTT 消息发送给数据上传线程
    let (mqtt_message_sender, mqtt_message_receiver): (mpsc::Sender<paho_mqtt::Message>, mpsc::Receiver<paho_mqtt::Message>) = mpsc::channel();

    // 通过该通道向所有需要获知网络连接状态的线程发送网络连通或断开消息（连通：Some(0)，断开：None）
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
                    debug!("Cloud connected, query offline data...");
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
    // 数据流：
    //   联网时，原始数据处理线程->数据管理线程->MQTT发布线程（发布），如果发布失败，向数据库操作线程请求保存该数据。
    //   网络断开时，原始数据处理线程->数据管理线程->数据库操作线程（离线保存）。
    //   网络恢复时，离线数据处理线程通过数据库操作线程取出缓存的消息，发送给数据管理线程，数据管理线程将数据发给MQTT发布线程，收到发送成功的反馈
    // 后，向数据库操作线程请求删除对应 id 的消息。
    let data_manager_builder = thread::Builder::new().name("data_manager".into());
    let data_manager = data_manager_builder.spawn(move || {
        let mut id: u32;
        let mut cloud_is_connected = false;
        for datum in datum_receiver.iter() {
            info!("datum: {{ id: {}, type: {:?}, value: {:?} }}", datum.id, datum.datum_type, datum.value.msg);
            match datum.datum_type {
                DatumType::Notice => {
                    match datum.id {
                        0 => cloud_is_connected = false,    /* 网络断开 */
                        _ => {
                            cloud_is_connected = true;    /* 网络已连接 */
                            // 发送联网消息给离线数据处理线程
                            if let Err(err) = cloud_link_broken_msg_sender.send(Some(0)) {
                                error!("Error send cloud link broken msg: {}", err);
                            }
                        },
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
            } else if id == 0 {    /* 没联网，存入数据库（原始数据处理线程发过来的数据，id 为 0） */
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
            } else {
                // 没有联网且 id 不为 0 的情况下，理论上没有这种情况。
                error!("Get datum(id: {}) from offine_data_handle_thread when device is offline!", id);
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
                    //   需要接收联网消息的线程由数据管理模块和离线数据处理模块，为了确保离线数据操作线程在数据管理线程获知网络恢复的情况下才向数据管理线程
                    // 发送数据，此处仅通知数据管理线程，数据管理模块再通知离线数据操作线程。
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
    let (tx, rx): (mpsc::Sender<MsgReceiver>, mpsc::Receiver<MsgReceiver>) = mpsc::channel();
    let mqtt_pub_thread_builder = thread::Builder::new().name("mqtt_pub_thread".into());
    let mqtt_pub_thread = mqtt_pub_thread_builder
        .spawn(
            mqtt::closure::pub_closure(client, topic, tls, cloud_statue_announcement_sender_clone, tx, datum_publish_receiver, format_log,
            publish_result_sender, mqtt_message_receiver, server_addr)
        )
        .unwrap();

    let mqtt_sub_thread_builder = thread::Builder::new().name("mqtt_sub_thread".into());
    let mqtt_sub_thread = mqtt_sub_thread_builder
        .spawn(mqtt::closure::sub_closure(rx, downstream_msg_tx, cloud_statue_announcement_sender))
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
