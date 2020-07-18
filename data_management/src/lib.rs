pub mod data_management{
    pub struct DeviceData {
        pub device_serial_number: usize,
        pub timestamp_msec: u64,
        pub time_string: String,
        pub temperature: f32,
        pub humidity: f32,
        pub voltage: f32,
        pub rssi: u32,
        pub error_code: i32,
    }

    pub struct DeviceError {
        pub device_serial_number: usize,
        pub timestamp_msec: u64,
        pub time_string: String,
        pub error_code: i32,
    }

    pub mod data_base{
        pub fn init_data_base(path: &str, name: &str) -> Result<rusqlite::Connection, ()> {
            let full_path = String::from(path) + name;
            match rusqlite::Connection::open(&full_path) {
                Ok(conn) => Ok(conn),
                Err(_err) => Err(()),
            }
        }

        pub fn create_device_table(db: &rusqlite::Connection) -> Result<usize, ()> {
            let r = db.execute(
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
                Ok(changed) => Ok(changed),
                Err(_err) => Err(()),
            }
        }

        pub fn create_device_data_table(){}
        pub fn create_device_error_table(){}
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

        pub fn create_device(db: &rusqlite::Connection, dev: &Device) -> Result<usize, ()> {
            let r = db.execute(
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
        pub fn new(device_serial_number: usize, timestamp_msec: u64, time_string: &str, temperature: f32, humidity: f32,
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
        pub fn new(device_serial_number: usize, timestamp_msec: u64, time_string: &str, error_code: i32) -> DeviceError {
            DeviceError {
                device_serial_number: device_serial_number,
                timestamp_msec: timestamp_msec,
                time_string: String::from(time_string),
                error_code: error_code,
            }
        }
    }
}

pub fn init_data_base(path: &str, name: &str) -> Result<rusqlite::Connection, ()> {
    match crate::data_management::data_base::init_data_base(path, name) {
        Ok(conn) => Ok(conn),
        Err(_err) => Err(()),
    }
}
