use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedConfig {
    pub deployment: DeploymentConfig,
    pub transport: TransportConfig,
}

impl SharedConfig {
    pub fn new(mode: CompileMode) -> Self {
        Self {
            deployment: DeploymentConfig::new(mode),
            transport: TransportConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub mode: CompileMode,
    pub remote_model: RemoteModelConfig,
    pub local_model: Option<LocalModelConfig>,
}

impl DeploymentConfig {
    pub fn new(mode: CompileMode) -> Self {
        Self {
            mode,
            remote_model: RemoteModelConfig::default(),
            local_model: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompileMode {
    UserPreprocessRemoteCoreRemoteModel,
    UserRemotePreprocessCoreRemoteModel,
    UserPreprocessCoreRemoteModel,
}

impl CompileMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserPreprocessRemoteCoreRemoteModel => "up_xcy_m",
            Self::UserRemotePreprocessCoreRemoteModel => "u_xpcy_m",
            Self::UserPreprocessCoreRemoteModel => "upxcy_m",
        }
    }

    pub const fn has_remote_user_boundary(self) -> bool {
        matches!(
            self,
            Self::UserPreprocessRemoteCoreRemoteModel | Self::UserRemotePreprocessCoreRemoteModel
        )
    }

    pub const fn has_remote_model_boundary(self) -> bool {
        true
    }

    pub const fn preprocess_placement(self) -> PreprocessPlacement {
        match self {
            Self::UserPreprocessRemoteCoreRemoteModel | Self::UserPreprocessCoreRemoteModel => {
                PreprocessPlacement::UserSide
            }
            Self::UserRemotePreprocessCoreRemoteModel => PreprocessPlacement::ComputationSide,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreprocessPlacement {
    UserSide,
    ComputationSide,
}

impl FromStr for CompileMode {
    type Err = CompileModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "up_xcy_m" => Ok(Self::UserPreprocessRemoteCoreRemoteModel),
            "u_xpcy_m" => Ok(Self::UserRemotePreprocessCoreRemoteModel),
            "upxcy_m" => Ok(Self::UserPreprocessCoreRemoteModel),
            _ => Err(CompileModeParseError {
                value: value.to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileModeParseError {
    pub value: String,
}

impl std::fmt::Display for CompileModeParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "unsupported compile mode '{}'; expected 'up_xcy_m', 'u_xpcy_m', or 'upxcy_m'",
            self.value
        )
    }
}

impl std::error::Error for CompileModeParseError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportConfig {
    pub user_bridge: String,
    pub model_bridge: String,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            user_bridge: "passthrough".to_owned(),
            model_bridge: "passthrough".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteModelConfig {
    pub endpoint: ModelEndpoint,
}

impl Default for RemoteModelConfig {
    fn default() -> Self {
        Self {
            endpoint: ModelEndpoint {
                url: "https://model.example.invalid".to_owned(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelConfig {
    pub model_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEndpoint {
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::CompileMode;
    use std::str::FromStr;

    #[test]
    fn parses_initial_compile_modes() {
        assert_eq!(
            CompileMode::from_str("up_xcy_m"),
            Ok(CompileMode::UserPreprocessRemoteCoreRemoteModel)
        );
        assert_eq!(
            CompileMode::from_str("u_xpcy_m"),
            Ok(CompileMode::UserRemotePreprocessCoreRemoteModel)
        );
        assert_eq!(
            CompileMode::from_str("upxcy_m"),
            Ok(CompileMode::UserPreprocessCoreRemoteModel)
        );
    }
}
