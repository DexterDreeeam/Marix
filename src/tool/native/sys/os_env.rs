use std::env;
#[cfg(not(target_os = "windows"))]
use std::fs;
use std::path::{Component, Path, PathBuf};

use marix_common::{
    Arch, Platform, System,
    external::serde_json::{Value, json, to_string},
};
use marix_protocol::{ToolCategory, ToolPreview};

use super::super::parse_input;
use crate::ToolProgram;

const NAME: &str = "os_env";
const DESCRIPTION: &str = "Report a safe, allowlisted view of the local system environment.";
const INPUT_SCHEMA: &str =
    concat!(r#"{"type":"object","properties":{},"additionalProperties":true}"#,);

pub struct OsEnv;

impl ToolProgram for OsEnv {
    fn preview(&self) -> ToolPreview {
        ToolPreview {
            name: NAME.to_owned(),
            description: DESCRIPTION.to_owned(),
            category: ToolCategory::System,
            system: System {
                platform: Platform::All,
                arch: Arch::All,
            },
            input: INPUT_SCHEMA.to_owned(),
        }
    }

    fn invoke(&self, call: &str) -> String {
        let source = if call.trim().is_empty() { "{}" } else { call };
        let input: Value = match parse_input(source) {
            Ok(value) => value,
            Err(error) => {
                return Self::failure(format!("invalid input: {error}"));
            }
        };
        if !input.is_object() {
            return Self::failure("input must be a JSON object".to_owned());
        }

        to_string(&Self::collect()).unwrap_or_default()
    }
}

#[cfg(feature = "os_env")]
pub use self::OsEnv as SelectedTool;

// -- Private -- //

struct KnownPaths {
    desktop: Option<PathBuf>,
    documents: Option<PathBuf>,
    downloads: Option<PathBuf>,
    app_data: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    public: Option<PathBuf>,
    program_data: Option<PathBuf>,
}

impl OsEnv {
    fn collect() -> Value {
        let profile = Self::profile_path();
        let profile_text = profile.as_deref().and_then(Self::path_string);
        let paths = Self::known_paths(profile.as_deref());
        let current = env::current_dir()
            .ok()
            .and_then(Self::absolute_path)
            .as_deref()
            .and_then(Self::path_string);
        let temp = Self::absolute_path(env::temp_dir())
            .as_deref()
            .and_then(Self::path_string);

        json!({
            "system": {
                "os": env::consts::OS,
                "family": env::consts::FAMILY,
                "arch": env::consts::ARCH,
                "hostname": Self::hostname(),
            },
            "user": {
                "username": Self::username(),
                "profile": profile_text,
            },
            "paths": {
                "current": current,
                "temp": temp,
                "desktop": Self::optional_path_string(paths.desktop),
                "documents": Self::optional_path_string(paths.documents),
                "downloads": Self::optional_path_string(paths.downloads),
                "app_data": Self::optional_path_string(paths.app_data),
                "local_app_data": Self::optional_path_string(
                    paths.local_app_data,
                ),
                "public": Self::optional_path_string(paths.public),
                "program_data": Self::optional_path_string(
                    paths.program_data,
                ),
            },
        })
    }

    fn hostname() -> Option<String> {
        #[cfg(target_os = "windows")]
        let name = "COMPUTERNAME";
        #[cfg(not(target_os = "windows"))]
        let name = "HOSTNAME";

        Self::env_text(name)
    }

    fn username() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            Self::env_text("USERNAME")
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::env_text("USER").or_else(|| Self::env_text("LOGNAME"))
        }
    }

    fn profile_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            Self::env_path("USERPROFILE").or_else(|| {
                let drive = env::var_os("HOMEDRIVE")?;
                let path = env::var_os("HOMEPATH")?;
                let mut profile = PathBuf::from(drive);
                profile.push(path);
                Self::absolute_path(profile)
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::env_path("HOME")
        }
    }

    #[cfg(target_os = "windows")]
    fn known_paths(profile: Option<&Path>) -> KnownPaths {
        KnownPaths {
            desktop: profile.map(|path| path.join("Desktop")),
            documents: profile.map(|path| path.join("Documents")),
            downloads: profile.map(|path| path.join("Downloads")),
            app_data: Self::env_path("APPDATA"),
            local_app_data: Self::env_path("LOCALAPPDATA"),
            public: Self::env_path("PUBLIC"),
            program_data: Self::env_path("PROGRAMDATA"),
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn known_paths(profile: Option<&Path>) -> KnownPaths {
        KnownPaths {
            desktop: Self::user_dir(profile, "XDG_DESKTOP_DIR", "Desktop"),
            documents: Self::user_dir(profile, "XDG_DOCUMENTS_DIR", "Documents"),
            downloads: Self::user_dir(profile, "XDG_DOWNLOAD_DIR", "Downloads"),
            app_data: Self::env_path("XDG_DATA_HOME")
                .or_else(|| profile.map(|path| path.join(".local").join("share"))),
            local_app_data: Self::env_path("XDG_CACHE_HOME")
                .or_else(|| profile.map(|path| path.join(".cache"))),
            public: Self::user_dir(profile, "XDG_PUBLICSHARE_DIR", "Public"),
            program_data: None,
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn user_dir(profile: Option<&Path>, key: &str, fallback: &str) -> Option<PathBuf> {
        let profile = profile?;
        Self::read_user_dir(profile, key)
            .or_else(|| Some(profile.join(fallback)))
            .and_then(Self::absolute_path)
    }

    #[cfg(not(target_os = "windows"))]
    fn read_user_dir(profile: &Path, key: &str) -> Option<PathBuf> {
        let config = Self::env_path("XDG_CONFIG_HOME")
            .unwrap_or_else(|| profile.join(".config"))
            .join("user-dirs.dirs");
        let content = fs::read_to_string(config).ok()?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((candidate, value)) = line.split_once('=') else {
                continue;
            };
            if candidate.trim() != key {
                continue;
            }
            let value = Self::unquote(value.trim());
            return Self::expand_home(value, profile).and_then(Self::absolute_path);
        }
        None
    }

    #[cfg(not(target_os = "windows"))]
    fn unquote(value: &str) -> &str {
        if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            &value[1..value.len() - 1]
        } else {
            value
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn expand_home(value: &str, profile: &Path) -> Option<PathBuf> {
        let allowed_removed = value.replace("${HOME}", "").replace("$HOME", "");
        if allowed_removed.contains('$') || allowed_removed.contains('`') {
            return None;
        }

        let home = profile.to_string_lossy();
        let expanded = value.replace("${HOME}", &home).replace("$HOME", &home);
        if expanded == "~" {
            return Some(profile.to_path_buf());
        }
        if let Some(relative) = expanded.strip_prefix("~/") {
            return Some(profile.join(relative));
        }
        Some(PathBuf::from(expanded))
    }

    fn env_text(name: &str) -> Option<String> {
        env::var_os(name)
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
    }

    fn env_path(name: &str) -> Option<PathBuf> {
        env::var_os(name)
            .filter(|value| !value.is_empty())
            .and_then(|value| Self::absolute_path(PathBuf::from(value)))
    }

    fn absolute_path(path: impl AsRef<Path>) -> Option<PathBuf> {
        let path = path.as_ref();
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            env::current_dir().ok()?.join(path)
        };
        Some(Self::normalize_path(&absolute))
    }

    fn normalize_path(path: &Path) -> PathBuf {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    if matches!(
                        normalized.components().next_back(),
                        Some(Component::Normal(_))
                    ) {
                        normalized.pop();
                    }
                }
                other => normalized.push(other.as_os_str()),
            }
        }
        normalized
    }

    fn optional_path_string(path: Option<PathBuf>) -> Option<String> {
        path.as_deref().and_then(Self::path_string)
    }

    fn path_string(path: &Path) -> Option<String> {
        let text = path.to_string_lossy().into_owned();
        if text.is_empty() { None } else { Some(text) }
    }

    fn failure(message: String) -> String {
        to_string(&json!({ "error": message })).unwrap_or_default()
    }
}
