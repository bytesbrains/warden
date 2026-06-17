//! Tiny hand-rolled `--key value` parser (no `clap`, to stay within the pinned 1.83
//! toolchain). Every flag takes a value (a token not starting with `--`), so `--payload -`
//! reads stdin and a `--key` with a missing value is a clear error. No boolean flags are
//! needed by any command.

use std::collections::HashMap;

pub struct Args {
    vals: HashMap<String, String>,
}

impl Args {
    pub fn parse(argv: Vec<String>) -> Result<Args, String> {
        let mut vals = HashMap::new();
        let mut i = 0;
        while i < argv.len() {
            let key = argv[i]
                .strip_prefix("--")
                .ok_or_else(|| format!("expected a --flag, got {:?}", argv[i]))?
                .to_string();
            match argv.get(i + 1) {
                Some(v) if !v.starts_with("--") => {
                    vals.insert(key, v.clone());
                    i += 2;
                }
                _ => return Err(format!("--{key} requires a value")),
            }
        }
        Ok(Args { vals })
    }

    pub fn get(&self, k: &str) -> Option<&str> {
        self.vals.get(k).map(String::as_str)
    }

    pub fn require(&self, k: &str) -> Result<&str, String> {
        self.get(k).ok_or_else(|| format!("missing required --{k}"))
    }
}
