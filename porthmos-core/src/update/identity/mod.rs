use porthmos_update::{Product, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub product: Product,
    pub version: Version,
}

impl AppIdentity {
    pub fn new(product: Product, crate_version: &str) -> AppIdentity {
        AppIdentity { product, version: crate_version.parse().expect("the crate version is valid semver") }
    }

    pub fn user_agent(&self) -> String {
        format!("{}/{}", self.product.program(), self.version)
    }
}

#[cfg(test)]
mod tests;
