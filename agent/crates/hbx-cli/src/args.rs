use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Args {
    pub positional: Vec<String>,
    pub flags: HashMap<String, String>,
    pub bool_flags: Vec<String>,
}

impl Args {
    pub fn parse(args: &[String]) -> Self {
        let mut positional = Vec::new();
        let mut flags = HashMap::new();
        let mut bool_flags = Vec::new();

        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if let Some(stripped) = arg.strip_prefix("--") {
                let key = stripped.to_string();
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    flags.insert(key, args[i + 1].clone());
                    i += 2;
                } else {
                    bool_flags.push(key);
                    i += 1;
                }
            } else if let Some(stripped) = arg.strip_prefix('-') {
                let key = stripped.to_string();
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    flags.insert(key, args[i + 1].clone());
                    i += 2;
                } else {
                    bool_flags.push(key);
                    i += 1;
                }
            } else {
                positional.push(arg.clone());
                i += 1;
            }
        }

        Self {
            positional,
            flags,
            bool_flags,
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.flags.get(key).map(|s| s.as_str())
    }

    pub fn has(&self, key: &str) -> bool {
        self.bool_flags.contains(&key.to_string()) || self.flags.contains_key(key)
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }
}