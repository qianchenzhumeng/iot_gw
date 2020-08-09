use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;

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
