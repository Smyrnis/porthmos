use std::{collections::BTreeMap, ffi::OsString, fmt, path::PathBuf, sync::Arc};

use async_trait::async_trait;

use crate::{ConnectionForm, FileSystem, Prompter, ProtocolError, VfsPath};

#[derive(Clone, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub options: BTreeMap<String, String>,
}

impl Target {
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(String::as_str)
    }
}

impl fmt::Debug for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Target")
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("options", &self.options.keys().collect::<Vec<_>>())
            .finish()
    }
}

const LAUNCHABLE_EXTENSIONS: [&str; 4] = [".com", ".exe", ".bat", ".cmd"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    pub home: Option<PathBuf>,
    pub user: Option<String>,
    pub path: Option<OsString>,
    pub editor: Option<String>,
    pub path_extensions: Vec<String>,
}

impl Environment {
    pub fn from_process() -> Self {
        Self {
            home: Self::home_from(std::env::var_os("HOME"), std::env::var_os("USERPROFILE")),
            user: Self::user_from(std::env::var("USER").ok(), std::env::var("USERNAME").ok()),
            path: std::env::var_os("PATH"),
            editor: Self::editor_from(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok()),
            path_extensions: if cfg!(windows) {
                Self::path_extensions_from(std::env::var_os("PATHEXT"))
            } else {
                Vec::new()
            },
        }
    }

    pub fn path_extensions_from(pathext: Option<OsString>) -> Vec<String> {
        let listed: Vec<String> = pathext
            .map(|text| text.to_string_lossy().into_owned())
            .unwrap_or_default()
            .split(';')
            .map(str::trim)
            .filter(|extension| !extension.is_empty())
            .map(|extension| format!(".{}", extension.trim_start_matches('.').to_lowercase()))
            .filter(|extension| LAUNCHABLE_EXTENSIONS.contains(&extension.as_str()))
            .collect();
        if listed.is_empty() { LAUNCHABLE_EXTENSIONS.map(String::from).to_vec() } else { listed }
    }

    pub fn find_program(&self, name: &str) -> Option<PathBuf> {
        if name.contains(['/', '\\']) {
            return self.first_existing(PathBuf::from(name));
        }
        let path = self.path.as_deref()?;
        std::env::split_paths(path)
            .filter(|directory| !directory.as_os_str().is_empty())
            .find_map(|directory| self.first_existing(directory.join(name)))
    }

    fn first_existing(&self, base: PathBuf) -> Option<PathBuf> {
        let lowered = base.to_string_lossy().to_lowercase();
        let already_listed = self.path_extensions.is_empty()
            || self.path_extensions.iter().any(|extension| lowered.ends_with(extension));
        let as_given = already_listed.then(|| base.clone());
        let with_extensions = self.path_extensions.iter().map(|extension| {
            let mut extended = base.clone().into_os_string();
            extended.push(extension);
            PathBuf::from(extended)
        });
        as_given.into_iter().chain(with_extensions).find(|candidate| candidate.is_file())
    }

    pub fn home_from(home: Option<OsString>, user_profile: Option<OsString>) -> Option<PathBuf> {
        [home, user_profile].into_iter().flatten().find(|value| !value.is_empty()).map(PathBuf::from)
    }

    pub fn user_from(user: Option<String>, user_name: Option<String>) -> Option<String> {
        [user, user_name].into_iter().flatten().find(|value| !value.is_empty())
    }

    pub fn editor_from(visual: Option<String>, editor: Option<String>) -> Option<String> {
        [visual, editor].into_iter().flatten().find(|value| !value.trim().is_empty())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ShellInvocation {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
}

impl ShellInvocation {
    pub fn starting_in(mut self, directory: &VfsPath) -> Self {
        let Some(destination) = self.args.pop() else {
            return self;
        };
        let quoted = directory.to_string_lossy().replace('\'', "'\\''");
        self.args.push("-t".into());
        self.args.push(destination);
        self.args.push(format!("cd '{quoted}' && exec $SHELL -l").into());
        self
    }

    pub fn to_command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.args);
        command.envs(self.env.iter().map(|(key, value)| (key, value)));
        command
    }
}

impl fmt::Debug for ShellInvocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_env: Vec<(&OsString, &str)> = self.env.iter().map(|(key, _)| (key, "<redacted>")).collect();
        f.debug_struct("ShellInvocation")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("env", &redacted_env)
            .finish()
    }
}

#[async_trait]
pub trait Protocol: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn description(&self) -> &'static str {
        ""
    }
    fn limits_note(&self) -> Option<&'static str> {
        None
    }
    fn default_port(&self) -> u16;
    fn connection_form(&self) -> ConnectionForm {
        ConnectionForm::standard(self.default_port())
    }
    fn discover(&self, _env: &Environment) -> Result<Vec<Target>, ProtocolError> {
        Ok(Vec::new())
    }
    async fn connect(&self, target: &Target, prompter: &mut dyn Prompter)
    -> Result<Arc<dyn FileSystem>, ProtocolError>;
    fn shell_command(&self, _target: &Target, _env: &Environment) -> Option<ShellInvocation> {
        None
    }
}

#[cfg(test)]
mod tests;
