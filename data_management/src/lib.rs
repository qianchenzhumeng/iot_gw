
pub mod data_management{
    #[derive(Debug)]
    pub struct DeviceData {
        pub device_serial_number: u32,
        pub name: String,
        pub timestamp_msec: i64,
        pub time_string: String,
        pub temperature: f64,
        pub humidity: f64,
        pub voltage: f64,
        pub rssi: u32,
        pub error_code: i32,
    }

    pub struct DeviceError {
        pub device_serial_number: u32,
        pub name: String,
        pub timestamp_msec: u64,
        pub time_string: String,
        pub error_code: i32,
    }

    pub mod data_base{
        pub fn open_data_base(path: &str, name: &str) -> Result<rusqlite::Connection, ()> {
            let full_path = String::from(path) + name;
            match rusqlite::Connection::open(&full_path) {
                Ok(conn) => Ok(conn),
                Err(_err) => Err(()),
            }
        }
        pub fn close_data_base(db: rusqlite::Connection) -> Result<(), ()> {
            match db.close() {
                Ok(_ok) => Ok(()),
                Err(_err) => Err(()),
            }
        }

        pub fn create_device_table(db: &rusqlite::Connection) -> Result<(), ()> {
            let r = db.execute(
                    "CREATE TABLE DEVICE(
                        ID INTEGER PRIMARY KEY,
                        NAME CHAR(20),
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

        pub fn create_device_data_table(db: &rusqlite::Connection) -> Result<(), ()> {
            let r = db.execute(
                    "CREATE TABLE DEVICE_DATA(
                        ID INTEGER PRIMARY KEY,
                        DEVICE_ID INTEGER,
                        NAME CHAR(20),
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
        pub fn create_device_error_table(db: &rusqlite::Connection) -> Result<(), ()> {
            let r = db.execute(
                    "CREATE TABLE DEVICE_ERROR(
                        ID INTEGER PRIMARY KEY,
                        NAME CHAR(20),
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
        pub fn insert_data_to_device_data_table(db: &rusqlite::Connection, data: &crate::data_management::DeviceData) -> Result<usize, ()> {
            let r = db.execute(
                "INSERT INTO DEVICE_DATA(DEVICE_ID, NAME, TIMESTAMP, TIMESTRING, TEMPERATURE, HUMIDITY, VOLTAGE) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![data.device_serial_number, data.name, data.timestamp_msec, data.time_string,
                    data.temperature, data.humidity, data.voltage],
            );
            match r {
                Ok(inserted) => Ok(inserted),
                Err(_err) => Err(()),
            }
        }
        pub fn querry_device_data(db: &rusqlite::Connection) -> Result<rusqlite::Statement, ()> {
            match db.prepare("SELECT * FROM DEVICE_DATA") {
                Ok(stmt) => Ok(stmt),
                Err(_err) => Err(()),
            }
        }
        pub fn delete_device_data(db: &rusqlite::Connection, id: u32) -> Result<usize, ()> {
            let r = db.execute(
                "DELETE FROM DEVICE_DATA WHERE ID =(?1)",
                rusqlite::params![id],
            );
            match r {
                Ok(deleted) => Ok(deleted),
                Err(_err) => Err(()),
            }
        }
    }

    pub mod device{
        #[derive(Debug)]
        pub struct Device {
            pub device_serial_number: u32,
            pub name: String,
            pub device_type: String,
            pub timestamp_msec: u32,
            pub time_string: String,
        }
        
        impl Device{
            pub fn new(device_serial_number: u32, name: &str,device_type: &str, timestamp_msec: u32, time_string: &str) -> Device {
                Device {
                    device_serial_number: device_serial_number,
                    name: String::from(name),
                    device_type: String::from(device_type),
                    timestamp_msec: timestamp_msec,
                    time_string: String::from(time_string),
                }
            }
        }

        pub fn create_device(db: &std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>, dev: &Device) -> Result<usize, ()> {
            let r = db.lock().unwrap().execute(
                "INSERT INTO DEVICE(SN, NAME, TYPE, TIME_MS, TIME_STR) VALUES(?1, ?2, ?3, ?4)",
                rusqlite::params![dev.device_serial_number, dev.name, dev.device_type, dev.timestamp_msec, dev.time_string],
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
        pub fn new(device_serial_number: u32, name: &str, timestamp_msec: i64, time_string: &str, temperature: f64, humidity: f64,
                    voltage: f64, rssi: u32, error_code: i32) -> DeviceData {
                        DeviceData {
                            device_serial_number: device_serial_number,
                            name: String::from(name),
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
        pub fn new(device_serial_number: u32, name: &str, timestamp_msec: u64, time_string: &str, error_code: i32) -> DeviceError {
            DeviceError {
                device_serial_number: device_serial_number,
                name: String::from(name),
                timestamp_msec: timestamp_msec,
                time_string: String::from(time_string),
                error_code: error_code,
            }
        }
    }
}
