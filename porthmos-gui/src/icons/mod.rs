use std::sync::OnceLock;

use iced::widget::svg::Handle;

macro_rules! icons {
    ($($variant:ident => $path:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon {
            $($variant),*
        }

        impl Icon {
            pub const ALL: &'static [Icon] = &[$(Icon::$variant),*];
        }

        pub fn bytes(icon: Icon) -> &'static [u8] {
            match icon {
                $(Icon::$variant => include_bytes!(concat!("../../assets/icons/", $path))),*
            }
        }
    };
}

icons! {
    Folder => "files/folder.svg",
    Parent => "files/upFolder.svg",
    File => "files/unknown.svg",
    Text => "files/text.svg",
    Markdown => "files/markdown.svg",
    Json => "files/json.svg",
    Yaml => "files/yaml.svg",
    Toml => "files/toml.svg",
    Xml => "files/xml.svg",
    Html => "files/html.svg",
    Xhtml => "files/xhtml.svg",
    Css => "files/css.svg",
    JavaScript => "files/javaScript.svg",
    TypeScript => "files/typeScript.svg",
    Java => "files/java.svg",
    Shell => "files/shell.svg",
    Sql => "files/sql.svg",
    Image => "files/image.svg",
    Archive => "files/archive.svg",
    Config => "files/config.svg",
    Properties => "files/properties.svg",
    Binary => "files/binaryData.svg",
    Font => "files/font.svg",
    Csv => "files/csv.svg",
    Docker => "files/docker.svg",
    Gradle => "files/gradle.svg",
    Groovy => "files/groovy.svg",
    Perl => "files/perl.svg",
    Patch => "files/patch.svg",
    GitIgnore => "files/gitignore.svg",
    Windows => "files/microsoftWindows.svg",
    Manifest => "files/manifest.svg",
    EditorConfig => "files/editorConfig.svg",
    Copy => "actions/copy.svg",
    Move => "actions/moveToRight.svg",
    Delete => "actions/delete.svg",
    Refresh => "actions/refresh.svg",
    Search => "actions/search.svg",
    Settings => "actions/settings.svg",
    Add => "actions/add.svg",
    More => "actions/moreHorizontal.svg",
    Up => "actions/up.svg",
    Back => "actions/chevronLeft.svg",
    Forward => "actions/chevronRight.svg",
    ChevronDown => "actions/chevronDown.svg",
    Close => "actions/closeSmall.svg",
    Transfers => "actions/upload.svg",
    Rename => "actions/edit.svg",
    NewFolder => "actions/addDirectory.svg",
}

static HANDLES: OnceLock<Vec<Handle>> = OnceLock::new();

pub fn handle(icon: Icon) -> Handle {
    let handles = HANDLES.get_or_init(|| Icon::ALL.iter().map(|icon| Handle::from_memory(bytes(*icon))).collect());
    handles[icon as usize].clone()
}

const EXACT_NAMES: &[(&str, Icon)] = &[
    ("dockerfile", Icon::Docker),
    ("docker-compose.yml", Icon::Docker),
    ("docker-compose.yaml", Icon::Docker),
    ("makefile", Icon::Config),
    (".gitignore", Icon::GitIgnore),
    (".gitattributes", Icon::GitIgnore),
    (".dockerignore", Icon::GitIgnore),
    (".editorconfig", Icon::EditorConfig),
    ("manifest.mf", Icon::Manifest),
    ("cargo.toml", Icon::Toml),
    ("cargo.lock", Icon::Toml),
    ("build.gradle", Icon::Gradle),
];

const EXTENSIONS: &[(&str, Icon)] = &[
    ("md", Icon::Markdown),
    ("markdown", Icon::Markdown),
    ("json", Icon::Json),
    ("yml", Icon::Yaml),
    ("yaml", Icon::Yaml),
    ("toml", Icon::Toml),
    ("xml", Icon::Xml),
    ("xsd", Icon::Xml),
    ("wsdl", Icon::Xml),
    ("html", Icon::Html),
    ("htm", Icon::Html),
    ("xhtml", Icon::Xhtml),
    ("css", Icon::Css),
    ("scss", Icon::Css),
    ("less", Icon::Css),
    ("js", Icon::JavaScript),
    ("mjs", Icon::JavaScript),
    ("cjs", Icon::JavaScript),
    ("jsx", Icon::JavaScript),
    ("ts", Icon::TypeScript),
    ("tsx", Icon::TypeScript),
    ("java", Icon::Java),
    ("sh", Icon::Shell),
    ("bash", Icon::Shell),
    ("zsh", Icon::Shell),
    ("fish", Icon::Shell),
    ("sql", Icon::Sql),
    ("png", Icon::Image),
    ("jpg", Icon::Image),
    ("jpeg", Icon::Image),
    ("gif", Icon::Image),
    ("bmp", Icon::Image),
    ("webp", Icon::Image),
    ("ico", Icon::Image),
    ("tif", Icon::Image),
    ("tiff", Icon::Image),
    ("svg", Icon::Image),
    ("heic", Icon::Image),
    ("avif", Icon::Image),
    ("zip", Icon::Archive),
    ("tar", Icon::Archive),
    ("gz", Icon::Archive),
    ("tgz", Icon::Archive),
    ("bz2", Icon::Archive),
    ("xz", Icon::Archive),
    ("7z", Icon::Archive),
    ("rar", Icon::Archive),
    ("zst", Icon::Archive),
    ("jar", Icon::Archive),
    ("ini", Icon::Config),
    ("cfg", Icon::Config),
    ("conf", Icon::Config),
    ("env", Icon::Config),
    ("properties", Icon::Properties),
    ("exe", Icon::Binary),
    ("dll", Icon::Binary),
    ("bin", Icon::Binary),
    ("so", Icon::Binary),
    ("dylib", Icon::Binary),
    ("o", Icon::Binary),
    ("class", Icon::Binary),
    ("iso", Icon::Binary),
    ("img", Icon::Binary),
    ("ttf", Icon::Font),
    ("otf", Icon::Font),
    ("woff", Icon::Font),
    ("woff2", Icon::Font),
    ("csv", Icon::Csv),
    ("tsv", Icon::Csv),
    ("gradle", Icon::Gradle),
    ("kts", Icon::Gradle),
    ("groovy", Icon::Groovy),
    ("pl", Icon::Perl),
    ("pm", Icon::Perl),
    ("patch", Icon::Patch),
    ("diff", Icon::Patch),
    ("bat", Icon::Windows),
    ("cmd", Icon::Windows),
    ("ps1", Icon::Windows),
    ("msi", Icon::Windows),
    ("txt", Icon::Text),
    ("log", Icon::Text),
    ("rs", Icon::Text),
    ("py", Icon::Text),
    ("go", Icon::Text),
    ("c", Icon::Text),
    ("cc", Icon::Text),
    ("cpp", Icon::Text),
    ("h", Icon::Text),
    ("hpp", Icon::Text),
    ("cs", Icon::Text),
    ("swift", Icon::Text),
    ("rb", Icon::Text),
    ("php", Icon::Text),
    ("kt", Icon::Text),
    ("lua", Icon::Text),
    ("dart", Icon::Text),
    ("scala", Icon::Text),
    ("r", Icon::Text),
    ("ex", Icon::Text),
    ("exs", Icon::Text),
    ("hs", Icon::Text),
    ("vue", Icon::Text),
    ("rst", Icon::Text),
    ("ipynb", Icon::Text),
    ("tex", Icon::Text),
    ("cmake", Icon::Config),
    ("lock", Icon::Config),
];

pub fn icon_for_entry(name: &str, is_dir: bool) -> Icon {
    if is_dir {
        return Icon::Folder;
    }
    let lowered = name.to_lowercase();
    if let Some((_, icon)) = EXACT_NAMES.iter().find(|(known, _)| *known == lowered) {
        return *icon;
    }
    let extension = match lowered.rfind('.') {
        Some(0) | None => return Icon::File,
        Some(dot) => &lowered[dot + 1..],
    };
    EXTENSIONS.iter().find(|(known, _)| *known == extension).map(|(_, icon)| *icon).unwrap_or(Icon::File)
}

#[cfg(test)]
mod tests;
