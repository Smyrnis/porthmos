pub fn running_target() -> &'static str {
    env!("PORTHMOS_TARGET")
}

#[cfg(test)]
mod tests;
