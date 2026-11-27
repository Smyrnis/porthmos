use crate::probe::{PRELUDE, Token, with_token};

const BODY: &str = r#"m uid
id -u 2>/dev/null
m tcp
head -n 1 /proc/net/tcp 2>/dev/null
grep ' 0A ' /proc/net/tcp 2>/dev/null | head -n 2000
grep -v ' 0A ' /proc/net/tcp 2>/dev/null | tail -n +2 | head -n 2000
m tcp6
head -n 1 /proc/net/tcp6 2>/dev/null
grep ' 0A ' /proc/net/tcp6 2>/dev/null | head -n 2000
grep -v ' 0A ' /proc/net/tcp6 2>/dev/null | tail -n +2 | head -n 2000
m udp
head -n 1 /proc/net/udp 2>/dev/null
grep ' 07 ' /proc/net/udp 2>/dev/null | head -n 2000
grep -v ' 07 ' /proc/net/udp 2>/dev/null | tail -n +2 | head -n 2000
m udp6
head -n 1 /proc/net/udp6 2>/dev/null
grep ' 07 ' /proc/net/udp6 2>/dev/null | head -n 2000
grep -v ' 07 ' /proc/net/udp6 2>/dev/null | tail -n +2 | head -n 2000
m netdev
head -n 200 /proc/net/dev 2>/dev/null
m sysnet
for d in /sys/class/net/*; do
  [ -d "$d" ] || continue
  printf '%s|%s|%s|%s\n' "${d##*/}" "$(cat "$d/operstate" 2>/dev/null)" "$(cat "$d/address" 2>/dev/null)" "$(cat "$d/mtu" 2>/dev/null)"
done | head -n 200
m ipaddr
ip -o addr 2>/dev/null | head -n 500
m comm
grep -s '' /proc/[0-9]*/comm 2>/dev/null | head -c 100000
m fdlinks
ls -l /proc/[0-9]*/fd 2>/dev/null | grep -E '^/proc|socket:\[' | head -c 300000
m end
"#;

pub fn ports_command(token: &Token) -> String {
    with_token(&format!("{PRELUDE}{BODY}"), token)
}
