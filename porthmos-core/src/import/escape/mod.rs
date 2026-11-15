const UTF8_MARKER: [u8; 3] = [0xEF, 0xBB, 0xBF];

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn decode(text: &str) -> String {
    let input = text.as_bytes();
    let mut bytes = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let escaped = match (input[index], input.get(index + 1), input.get(index + 2)) {
            (b'%', Some(&high), Some(&low)) => hex_value(high).zip(hex_value(low)).map(|(high, low)| high * 16 + low),
            _ => None,
        };
        match escaped {
            Some(byte) => {
                bytes.push(byte);
                index += 3;
            }
            None => {
                bytes.push(input[index]);
                index += 1;
            }
        }
    }
    let body = bytes.strip_prefix(&UTF8_MARKER).unwrap_or(&bytes);
    String::from_utf8_lossy(body).into_owned()
}

#[cfg(test)]
mod tests;
