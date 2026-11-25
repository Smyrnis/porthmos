use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
};

const PLACEHOLDER: &str = "@@T:";
const MAX_TOKEN_LENGTH: usize = 64;

const SCRIPT: &str = r#"m() { printf '\n@@T:%s\n' "$1"; }
LC_ALL=C; export LC_ALL
os=$(uname -s 2>/dev/null)
m os
echo "$os"
if [ "$os" != Linux ]; then
  m end
  exit 0
fi
m uid
id -u 2>/dev/null
m hostname
cat /proc/sys/kernel/hostname 2>/dev/null || hostname 2>/dev/null
m osrelease
head -n 40 /etc/os-release 2>/dev/null
m kernel
cat /proc/sys/kernel/osrelease 2>/dev/null
m uptime
cat /proc/uptime 2>/dev/null
m loadavg
cat /proc/loadavg 2>/dev/null
m meminfo
head -n 60 /proc/meminfo 2>/dev/null
m stat
grep '^cpu' /proc/stat 2>/dev/null | head -n 4097
m netdev
head -n 200 /proc/net/dev 2>/dev/null
m df
df -P -k 2>/dev/null | head -n 200
m passwd
cut -d: -f1,3 /etc/passwd 2>/dev/null | head -n 2000
m pagesize
getconf PAGESIZE 2>/dev/null || echo 4096
m status
grep -Hs '^Uid:' /proc/[0-9]*/status 2>/dev/null | head -c 150000
m cmdline
grep -aHs '' /proc/[0-9]*/cmdline 2>/dev/null | tr '\0' ' ' | head -c 300000
m procs
grep -aHs '' /proc/[0-9]*/stat 2>/dev/null | head -c 350000
m end
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    pub fn new() -> Self {
        let mut text = String::new();
        for _ in 0..2 {
            let mut hasher = RandomState::new().build_hasher();
            hasher.write_u8(0);
            text.push_str(&format!("{:016x}", hasher.finish()));
        }
        Self(text)
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let valid = !text.is_empty() && text.len() <= MAX_TOKEN_LENGTH && text.bytes().all(|b| b.is_ascii_hexdigit());
        valid.then(|| Self(text.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Token {
    fn default() -> Self {
        Self::new()
    }
}

pub fn probe_command(token: &Token) -> String {
    SCRIPT.replace(PLACEHOLDER, &format!("@@{}:", token.as_str()))
}

#[cfg(test)]
mod tests;
