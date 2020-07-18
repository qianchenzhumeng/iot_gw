extern crate data_management;
use data_management::data_management::{data_base, device};

fn main() {
    let db = data_base::init_data_base("./", "a.db");

    let db = match db {
        Ok(database) => database,
        Err(err) => {
            panic!("Problem opening the database: {:?}", err)
        },
    };

    match data_base::create_device_table(&db) {
        Ok(changed) => println!("{} rows were changed", changed),
        Err(err) => println!("create DEVICE table failed: {:?}", err),
    }

    let dev = device::Device {
        device_serial_number: 0,
        device_type: "I".to_string(),
        timestamp_msec: 0,
        time_string: "0000".to_string(),
    };

    match device::create_device(&db, &dev) {
        Ok(created) => println!("{} device were created", created),
        //Err(err) => println!("create device {:#?} failed: {:?}", dev, err),
        Err(err) => {},
    };
    
    let _db = match db.close() {
        Ok(database) => database,
        Err(err) => {
            panic!("Problem closing the database: {:?}", err)
        },
    };
}
