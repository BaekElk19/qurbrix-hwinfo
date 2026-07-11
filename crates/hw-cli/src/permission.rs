use crate::args::Command;
use anyhow::bail;

pub fn command_requires_hardware_access(command: &Command) -> bool {
    matches!(
        command,
        Command::Scan(_) | Command::Summary | Command::Table(_) | Command::BindId(_)
    )
}

pub fn ensure_root() -> anyhow::Result<()> {
    ensure_root_with(|| unsafe { libc::geteuid() })
}

pub fn ensure_root_with(geteuid: impl FnOnce() -> u32) -> anyhow::Result<()> {
    let uid = geteuid();
    if uid == 0 {
        return Ok(());
    }

    bail!("root access is required for this command; rerun with sudo")
}
