use std::env;

use crate::external::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    All,
    // Minimum supported version: Windows 10 22H2.
    Win,
    // Minimum supported version: Ubuntu 22.04 LTS.
    Ubuntu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    /// Both supported 64-bit architecture families; 32-bit is not supported.
    All,
    /// AMD64/x86_64; 32-bit x86 is not supported.
    Amd,
    /// ARM64/AArch64; 32-bit ARM is not supported.
    Arm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct System {
    pub platform: Platform,
    pub arch: Arch,
}

impl System {
    pub fn new() -> Self {
        let platform = if cfg!(target_os = "windows") {
            Platform::Win
        } else if cfg!(target_os = "linux") && Self::is_ubuntu_host() {
            Platform::Ubuntu
        } else {
            panic!("unsupported platform: {}", env::consts::OS);
        };

        let arch = if cfg!(target_arch = "x86_64") {
            Arch::Amd
        } else if cfg!(target_arch = "aarch64") {
            Arch::Arm
        } else {
            panic!("unsupported architecture: {}", env::consts::ARCH);
        };

        Self { platform, arch }
    }

    pub fn supports(&self, host: &System) -> bool {
        (self.platform == Platform::All || self.platform == host.platform)
            && (self.arch == Arch::All || self.arch == host.arch)
    }
}

// -- Private -- //

impl System {
    fn is_ubuntu_host() -> bool {
        std::fs::read_to_string("/etc/os-release")
            .map(|content| {
                content
                    .lines()
                    .any(|line| line == "ID=ubuntu" || line == "ID=\"ubuntu\"")
            })
            .unwrap_or(false)
    }
}
