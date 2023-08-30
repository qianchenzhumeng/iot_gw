pub mod data_management {
    #[derive(Debug)]
    pub struct DeviceData {
        pub msg: String,
    }

    pub mod data_base {
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

        pub fn create_device_data_table(db: &rusqlite::Connection) -> Result<(), ()> {
            let r = db.execute(
                "CREATE TABLE DEVICE_DATA(
                        ID INTEGER PRIMARY KEY,
                        MSG CHAR(256)
                    )",
                rusqlite::params![],
            );
            match r {
                Ok(_ok) => Ok(()),
                Err(_err) => Err(()),
            }
        }

        pub fn device_data_table_exsits(db: &rusqlite::Connection) -> bool {
            match db
                .prepare("SELECT * FROM sqlite_master WHERE name='DEVICE_DATA' and type='table'")
            {
                Ok(mut stmt) => match stmt.exists(rusqlite::NO_PARAMS) {
                    Ok(r) => r,
                    Err(_) => false,
                },
                Err(_err) => false,
            }
        }

        pub fn insert_data_to_device_data_table(
            db: &rusqlite::Connection,
            data: &super::DeviceData,
        ) -> Result<usize, ()> {
            let r = db.execute(
                "INSERT INTO DEVICE_DATA(MSG) VALUES(?1)",
                rusqlite::params![data.msg],
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

    impl DeviceData {
        pub fn new(msg: &str) -> DeviceData {
            DeviceData {
                msg: String::from(msg),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data_manager::data_management::data_base;
    #[test]
    fn it_works() {
        let db = data_base::open_data_base("./", "test.db");
        match db {
            Ok(db) => {
                match data_base::create_device_data_table(&db) {
                    _ => {}
                }
                assert_eq!(data_base::device_data_table_exsits(&db), true);
            }
            Err(err) => {
                panic!("Problem opening the database: {:?}", err)
            }
        }
    }
}
