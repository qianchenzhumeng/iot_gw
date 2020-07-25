pub mod file_if{
    pub fn read_msg(filename: &str) -> Result<String, ()> {
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