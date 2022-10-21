extern crate min_rs as min;
extern crate log;

use std::io::prelude::*;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use log::{debug, trace};
use serialport::SerialPort;

#[derive(Debug,Copy,Clone)]
pub struct FileIf;

impl FileIf {
    pub fn read(self, filename: &str) -> Result<String, ()> {
        match std::fs::read_to_string(filename) {
            Ok(msg) => {
                match std::fs::write(filename, "") {
                    _ => {},
                };
                Ok(msg)
            },
            Err(_err) => Err(()),
        }
    }
}

pub struct HwIf {
    port: RefCell<Box<dyn SerialPort>>,
    name: String,
    tx_space_avaliable: u16,
    output: Arc<Mutex<String>>,
}

impl HwIf {
    pub fn new(port: Box<dyn SerialPort>, name: String, tx_space_avaliable: u16) -> Self {
        HwIf {
            port: RefCell::new(port),
            name: name,
            tx_space_avaliable: tx_space_avaliable,
            output: Arc::new(Mutex::new(String::from(""))),
        }
    }

    fn available_for_write(&self) -> u16 {
        self.tx_space_avaliable
    }

    fn tx(&self, byte: u8) {
        let mut output = self.output.lock().unwrap();
        output.push_str(format!("0x{:02x} ", byte).as_str());
        let mut port = self.port.borrow_mut();
        match port.write(&[byte]) {
            Ok(_) => {},
            Err(e) => {
                debug!(target: self.name.as_str(), "{}", e);
            },
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut port = self.port.borrow_mut();
        match port.read(&mut buf[..]) {
            Ok(n) => Ok(n),
            _ => Err(()),
        }
    }
}

impl min::Interface for HwIf {
    fn tx_start(&self) {
        let mut output = self.output.lock().unwrap();
        output.clear();
        output.push_str(format!("send frame: [ ").as_str());
    }
    
    fn tx_finished(&self) {
        let mut output = self.output.lock().unwrap();
        output.push_str(format!("]").as_str());
        trace!(target: self.name.as_str(), "{}", output);
    }
    fn tx_space(&self) -> u16 {
        self.available_for_write()
    }
    
    fn tx_byte(&self, _min_port: u8, byte: u8) {
        self.tx(byte);
    }
}
