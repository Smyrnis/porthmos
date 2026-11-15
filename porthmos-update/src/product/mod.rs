use semver::Version;

const RELEASES: &str = "https://github.com/Smyrnis/porthmos/releases/download";
const PREVIEW_CHANNEL: &str = "dev";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    Tui,
    Gui,
}

impl Product {
    pub fn id(self) -> &'static str {
        match self {
            Product::Tui => "tui",
            Product::Gui => "gui",
        }
    }

    pub fn program(self) -> &'static str {
        match self {
            Product::Tui => "porthmos",
            Product::Gui => "porthmos-gui",
        }
    }

    pub fn program_file(self) -> String {
        if cfg!(windows) { format!("{}.exe", self.program()) } else { self.program().to_string() }
    }

    pub fn tag_letter(self) -> char {
        match self {
            Product::Tui => 'T',
            Product::Gui => 'G',
        }
    }

    pub fn from_id(id: &str) -> Option<Product> {
        [Product::Tui, Product::Gui].into_iter().find(|product| product.id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Preview,
}

impl Channel {
    pub fn of(version: &Version) -> Channel {
        let mut parts = version.pre.as_str().split('.');
        let name = parts.next();
        let build = parts.next();
        let is_preview = name == Some(PREVIEW_CHANNEL)
            && parts.next().is_none()
            && build.is_none_or(|build| build.bytes().all(|byte| byte.is_ascii_digit()));
        if is_preview { Channel::Preview } else { Channel::Stable }
    }

    pub fn id(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Preview => "preview",
        }
    }
}

pub fn feed_release(product: Product, channel: Channel) -> String {
    format!("{}-{}", product.id(), channel.id())
}

pub fn manifest_url_for(product: Product, running: &Version) -> String {
    format!("{RELEASES}/{}/latest.json", feed_release(product, Channel::of(running)))
}

#[cfg(test)]
mod tests;
