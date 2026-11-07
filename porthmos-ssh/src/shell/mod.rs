use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use porthmos_vfs::{ShellInvocation, Target};

use crate::{
    askpass::{HELPER_SWITCH, PASSWORD_VARIABLE, USER_VARIABLE, usable_password},
    version::SshVersion,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PasswordLogin {
    Askpass { helper: PathBuf },
    Sshpass(PathBuf),
    Plain,
}

pub(crate) fn password_login(
    password: Option<&str>, helper: Option<PathBuf>, version: Option<SshVersion>, sshpass: Option<PathBuf>,
) -> PasswordLogin {
    let Some(password) = password else {
        return PasswordLogin::Plain;
    };
    let askpass_possible = usable_password(password) && version.is_some_and(SshVersion::supports_askpass_password);
    match (helper, sshpass) {
        (Some(helper), _) if askpass_possible => PasswordLogin::Askpass { helper },
        (_, Some(sshpass)) => PasswordLogin::Sshpass(sshpass),
        _ => PasswordLogin::Plain,
    }
}

pub(crate) fn live_helper(helper: Option<PathBuf>) -> Option<PathBuf> {
    helper.filter(|path| path.is_file())
}

fn env_pair(name: &str, value: impl Into<OsString>) -> (OsString, OsString) {
    (OsString::from(name), value.into())
}

pub fn invocation_for(target: &Target, ssh_config_alias: Option<&str>, login: PasswordLogin) -> ShellInvocation {
    let (program, mut args, env) = match (&target.password, login) {
        (Some(password), PasswordLogin::Sshpass(sshpass_path)) => (
            sshpass_path.into_os_string(),
            vec![OsString::from("-e"), OsString::from("ssh")],
            vec![(OsString::from("SSHPASS"), OsString::from(password))],
        ),
        (Some(password), PasswordLogin::Askpass { helper }) => (
            OsString::from("ssh"),
            vec![OsString::from("-o"), OsString::from("NumberOfPasswordPrompts=1")],
            vec![
                env_pair("SSH_ASKPASS", helper.into_os_string()),
                env_pair("SSH_ASKPASS_REQUIRE", "force"),
                env_pair(HELPER_SWITCH, "1"),
                env_pair(USER_VARIABLE, target.username.as_str()),
                env_pair(PASSWORD_VARIABLE, password.as_str()),
            ],
        ),
        _ => (OsString::from("ssh"), Vec::new(), Vec::new()),
    };

    if ssh_config_alias.is_some() {
        args.push(OsString::from("-o"));
        args.push(OsString::from(format!("HostName={}", target.host)));
    }
    args.push(OsString::from("-p"));
    args.push(OsString::from(target.port.to_string()));
    if ssh_config_alias.is_some() {
        args.push(OsString::from("-l"));
        args.push(OsString::from(&target.username));
    }
    if let Some(identity_file) = target.option("identity_file") {
        args.push(OsString::from("-i"));
        args.push(OsString::from(identity_file));
    }
    match ssh_config_alias {
        Some(alias) => args.push(OsString::from(alias)),
        None => args.push(OsString::from(format!("{}@{}", target.username, target.host))),
    }
    ShellInvocation { program, args, env }
}

#[cfg(unix)]
pub fn find_sshpass_in(path_var: &OsStr) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    std::env::split_paths(path_var).map(|dir| dir.join("sshpass")).find(|candidate| {
        candidate.is_file()
            && candidate.metadata().map(|metadata| metadata.permissions().mode() & 0o111 != 0).unwrap_or(false)
    })
}

#[cfg(windows)]
pub fn find_sshpass_in(_path_var: &OsStr) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests;
