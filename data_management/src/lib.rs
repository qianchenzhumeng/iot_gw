
pub mod data_management{
    #[derive(Debug)]
    pub struct DeviceData {
        pub device_serial_number: u32,
        pub timestamp_msec: u64,
        pub time_string: String,
        pub temperature: f32,
        pub humidity: f32,
        pub voltage: f32,
        pub rssi: u32,
        pub error_code: i32,
    }

    pub struct DeviceError {
        pub device_serial_number: u32,
        pub timestamp_msec: u64,
        pub time_string: String,
        pub error_code: i32,
    }

    pub mod data_base{
        pub fn open_data_base(path: &str, name: &str) -> Result<std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, ()> {
            let full_path = String::from(path) + name;
            match rusqlite::Connection::open(&full_path) {
                Ok(conn) => {
                    let conn = std::sync::Arc::new(std::sync::Mutex::new(conn));
                    Ok(conn)
                },
                Err(_err) => Err(()),
            }
        }
        //pub fn close_data_base(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>) -> Result<(), ()> {
        //    let db = db.lock().unwrap();
        //    match db.close() {
        //        Ok(_ok) => Ok(()),
        //        Err(_err) => Err(()),
        //    }
        //}

        pub fn create_device_table(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>) -> Result<(), ()> {
            let r = db.lock().unwrap().execute(
                    "CREATE TABLE DEVICE(
                        ID INTEGER PRIMARY KEY,
                        SN INTEGER,
                        TYPE CHAR(10),
                        TIME_MS INTEGER,
                        TIME_STR CHAR(10)
                    )",
                    rusqlite::params![],
                );
            match r {
                Ok(_ok) => Ok(()),
                Err(_err) => Err(()),
            }
        }

        pub fn create_device_data_table(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>) -> Result<(), ()> {
            let r = db.lock().unwrap().execute(
                    "CREATE TABLE DEVICE_DATA(
                        ID INTEGER PRIMARY KEY,
                        DEVICE_ID INTEGER,
                        TIMESTAMP INTEGER,
                        TIMESTRING CHAR(20),
                        TEMPERATURE REAL,
                        HUMIDITY REAL,
                        VOLTAGE REAL
                    )",
                    rusqlite::params![],
            );
            match r {
                Ok(_ok) => Ok(()),
                Err(_err) => Err(()),
            }
        }
        pub fn create_device_error_table(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>) -> Result<(), ()> {
            let r = db.lock().unwrap().execute(
                    "CREATE TABLE DEVICE_ERROR(
                        ID INTEGER PRIMARY KEY,
                        DEVICE_ID INTEGER,
                        TIMESTAMP INTEGER,
                        TIMESTRING CHAR(20),
                        ERROR_CODE INTEGER
                    )",
                    rusqlite::params![],
                );
            match r {
                Ok(_ok) => Ok(()),
                Err(_err) => Err(()),
            }
        }
        pub fn insert_data_to_device_data_table(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, data: &crate::data_management::DeviceData) -> Result<usize, ()> {
            let r = db.lock().unwrap().execute(
                "INSERT INTO DEVICE_DATA(DEVICE_ID, TIMESTAMP, TIMESTRING, TEMPERATURE, HUMIDITY, VOLTAGE) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![data.device_serial_number, data.timestamp_msec as i64, data.time_string,
                    data.temperature as f64, data.humidity as f64, data.voltage as f64
                ],
            );
            match r {
                Ok(inserted) => Ok(inserted),
                Err(_err) => Err(()),
            }
        }
    }

    pub mod device{
        #[derive(Debug)]
        pub struct Device {
            pub device_serial_number: u32,
            pub device_type: String,
            pub timestamp_msec: u32,
            pub time_string: String,
        }
        
        impl Device{
            pub fn new(device_serial_number: u32, device_type: &str, timestamp_msec: u32, time_string: &str) -> Device {
                Device {
                    device_serial_number: device_serial_number,
                    device_type: String::from(device_type),
                    timestamp_msec: timestamp_msec,
                    time_string: String::from(time_string),
                }
            }
        }

        pub fn create_device(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, dev: &Device) -> Result<usize, ()> {
            let r = db.lock().unwrap().execute(
                "INSERT INTO DEVICE(SN, TYPE, TIME_MS, TIME_STR) VALUES(?1, ?2, ?3, ?4)",
                rusqlite::params![dev.device_serial_number, dev.device_type, dev.timestamp_msec, dev.time_string],
            );
            match r {
                Ok(inserted) => Ok(inserted),
                Err(_err) => Err(()),
            }
        }
        pub fn store_device_data(){}
        pub fn store_device_error(){}
    }

    impl DeviceData {
        pub fn new(device_serial_number: u32, timestamp_msec: u64, time_string: &str, temperature: f32, humidity: f32,
                    voltage: f32, rssi: u32, error_code: i32) -> DeviceData {
                        DeviceData {
                            device_serial_number: device_serial_number,
                            timestamp_msec: timestamp_msec,
                            time_string: String::from(time_string),
                            temperature: temperature,
                            humidity: humidity,
                            voltage: voltage,
                            rssi: rssi,
                            error_code: error_code,
                        }
                    }
    }

    impl DeviceError {
        pub fn new(device_serial_number: u32, timestamp_msec: u64, time_string: &str, error_code: i32) -> DeviceError {
            DeviceError {
                device_serial_number: device_serial_number,
                timestamp_msec: timestamp_msec,
                time_string: String::from(time_string),
                error_code: error_code,
            }
        }
    }
}
