extern crate json;
use regex::Regex;

#[derive(Debug)]
pub struct Template<'a> {
    template: &'a str,
}

#[derive(Debug)]
pub enum Model {
    Value(String),
}

#[derive(Debug)]
pub enum Value {
    String(String),
    Number(i64),
}

#[derive(Debug)]
pub enum Error {
    RegexError,
    ParseError,
    CallError,
}

#[derive(Debug)]
enum CallType {
    GetTimestamp,
    Unknown,
}

pub type Models = Vec<Model>;

impl<'a> Template<'a> {
    pub fn new(template: &'a str) -> Self {
        Template {
            template: template,
        }
    }
    // <{ label }>
    pub fn get_value_models(&self) -> Result<Models, Error> {
        let re = match Regex::new(r"<\{\s*([^%>]+)\s*\}>") {
            Ok(re) => re,
            Err(_err) => return Err(crate::Error::RegexError),
        };
        let models: Vec<Model> = re.captures_iter(self.template).map(|caps| Model::Value(caps[0].to_string())).collect();
        Ok(models)
    }
    // <# TS #>
    pub fn get_call_models(&self) -> Result<Models, Error> {
        let re = match Regex::new(r"<#\s*([^%>]+)\s*#>") {
            Ok(re) => re,
            Err(_err) => return Err(crate::Error::RegexError),
        };
        let models: Vec<Model> = re.captures_iter(self.template).map(|caps| Model::Value(caps[0].to_string())).collect();
        Ok(models)
    }
}

impl Model {
    pub fn is_label(&self) -> bool {
        let re = match Regex::new(r"[^<\{\}\s%>]+") {
            Ok(re) => re,
            Err(_err) => return false,
        };
        let model = match self {
            Model::Value(model) => model,
        };
        match re.captures(&model) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn is_call(&self) -> bool {
        let re = match Regex::new(r"<#\s*([^%>]+)\s*#>") {
            Ok(re) => re,
            Err(_err) => return false,
        };
        let model = match self {
            Model::Value(model) => model,
        };
        match re.captures(&model) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn get_label(&self) -> Result<String, Error> {
        if let false = self.is_label() {
            return Err(crate::Error::ParseError);
        }
        let re = match Regex::new(r"[^<\{\}\s%>]+") {
            Ok(re) => re,
            Err(_err) => return Err(crate::Error::RegexError),
        };
        let model = match self {
            Model::Value(model) => model,
        };
        let label = match re.captures(&model) {
            Some(cap) => {
                cap[0].to_string()
            },
            None => "".to_string(),
        };
        Ok(label)
    }

    fn get_call_type(&self) -> Result<crate::CallType, Error> {
        if let false = self.is_call() {
            return Err(crate::Error::ParseError);
        }
        let re = match Regex::new(r"[^<#\}\s#>]+") {
            Ok(re) => re,
            Err(_err) => return Err(crate::Error::RegexError),
        };
        let model = match self {
            Model::Value(model) => model,
        };
        let s = match re.captures(&model) {
            Some(cap) => {
                cap[0].to_string()
            },
            None => "".to_string(),
        };
        if s.eq("TS") {
            Ok(crate::CallType::GetTimestamp)
        } else {
            Ok(crate::CallType::Unknown)
        }
    }

    fn get_timestamp_msec(&self) -> Result<i64, Error> {
        let n = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(n) => n,
            Err(_) => std::time::Duration::from_secs(0),
        };
        Ok(n.as_millis() as i64)
    }

    pub fn get_call_result(&self) -> Result<crate::Value, Error> {
        if let false = self.is_call() {
            return Err(crate::Error::ParseError);
        }
        if let Err(_) = Regex::new(r"<#\s*([^%>]+)\s*#>") {
            return Err(crate::Error::ParseError);
        };
        match self.get_call_type() {
            Ok(call_type) => {
                match call_type {
                    crate::CallType::GetTimestamp => {
                        if let Ok(timestamp_msec) = self.get_timestamp_msec() {
                            return Ok(crate::Value::Number(timestamp_msec));
                        } else {
                            return Err(crate::Error::CallError);
                        }
                    }
                    _ => Err(crate::Error::CallError),
                }
            },
            Err(err) => Err(err),
        }
    }
}
