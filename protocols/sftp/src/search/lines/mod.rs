#[derive(Default)]
pub(crate) struct LineSplitter {
    pending: Vec<u8>,
}

impl LineSplitter {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                if !self.pending.is_empty() {
                    lines.push(String::from_utf8_lossy(&self.pending).into_owned());
                    self.pending.clear();
                }
            } else {
                self.pending.push(byte);
            }
        }
        lines
    }

    pub(crate) fn finish(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        let line = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        Some(line)
    }
}

#[cfg(test)]
mod tests;
