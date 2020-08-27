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
    RegexError(regex::Error),
    ParseError,
    CallError,
}

#[derive(Debug)]
enum CallType {
    GetTimestamp,
    Unknown,
}

pub type Models = Vec<Model>;

impl From<regex::Error> for Error {
    fn from(error: regex::Error) -> Self {
        Error::RegexError(error)
    }
}

impl<'a> Template<'a> {
    pub fn new(template: &'a str) -> Self {
        Template {
            template: template,
        }
    }
    // <{ label }>
    pub fn get_value_models(&self) -> Result<Models, Error> {
        let re = Regex::new(r"<\{\s*([^%>]+)\s*\}>")?;
        let models: Vec<Model> = re.captures_iter(self.template).map(|caps| Model::Value(caps[0].to_string())).collect();
        Ok(models)
    }
    // <# TS #>
    pub fn get_call_models(&self) -> Result<Models, Error> {
        let re = Regex::new(r"<#\s*([^%>]+)\s*#>")?;
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
        let re = Regex::new(r"[^<\{\}\s%>]+")?;
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
        let re = Regex::new(r"[^<#\}\s#>]+")?;
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
        Regex::new(r"<#\s*([^%>]+)\s*#>")?;
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

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        match self {
            crate::Model::Value(self_value) => {
                match other {
                    crate::Model::Value(other_value) => {
                        if self_value != other_value {
                            return false;
                        } else {
                            return true;
                        }
                    },
                    // _ => false,
                }
            },
            // _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    fn models_eq(this: crate::Models, other: crate::Models) -> bool {
        if this.len() == other.len() {
            for i in 0..this.len() {
                if this[i] != other[i] {
                    println!("this: {:#?}, other: {:#?}", this, other);
                    return false;
                }
            }
            true
        } else {
            println!("this: {:#?}, other: {:#?}", this, other);
            false
        }
    }

    #[test]
    fn template_get_value_models() {
        let template = crate::Template::new("{<{name}>: [{\"ts\": <#TS#>,\"values\": <{value}>}]}");
        match template.get_value_models() {
            Ok(models) => {
                let v: Vec<crate::Model> = vec![
                    crate::Model::Value("<{name}>".to_string()),
                    crate::Model::Value("<{value}>".to_string()),
                ];
                assert_eq!(models_eq(models, v), true);
            },
            Err(_) => panic!("Template::get_value_models test failed"),
        }
    }

    #[test]
    fn template_get_call_models() {
        let template = crate::Template::new("{<{name}>: [{\"ts\": <#TS#>,\"values\": <{value}>}]}");
        match template.get_call_models() {
            Ok(models) => {
                let v: Vec<crate::Model> = vec![
                    crate::Model::Value("<#TS#>".to_string()),
                ];
                assert_eq!(models_eq(models, v), true);
            },
            Err(_) => panic!("Template::get_call_models test failed"),
        }
    }

    #[test]
    fn model_is_label() {
        assert_eq!(crate::Model::Value("<{name}>".to_string()).is_label(), true);
    }

    #[test]
    fn model_get_label() {
        match crate::Model::Value("<{name}>".to_string()).get_label() {
            Ok(label) => {
                assert_eq!(label, "name");
            }
            _ => panic!("Model::get_label test failed"),
        }
    }

    #[test]
    fn model_is_call() {
        assert_eq!(crate::Model::Value("<#TS#>".to_string()).is_call(), true);
    }

    #[test]
    fn model_get_call_type() {
        match crate::Model::Value("<#TS#>".to_string()).get_call_type(){
            Ok(call_type) => match call_type {
                crate::CallType::GetTimestamp => {},
                _ => panic!("<#TS#> should be CallType::GetTimestamp"),
            },
            Err(_) => panic!("<#TS#> should be CallType::GetTimestamp"),
        }
    }

    #[test]
    fn model_get_timestamp_msec() {
        match crate::Model::Value("<#TS#>".to_string()).get_timestamp_msec(){
            Ok(_) => {},
            Err(_) => panic!("Model::get_timestamp_msec test failed"),
        }
    }

    #[test]
    fn model_get_call_result() {
        match crate::Model::Value("<#TS#>".to_string()).get_call_result(){
            Ok(_) => {},
            Err(_) => panic!("Model::get_call_result test failed"),
        }
    }
}