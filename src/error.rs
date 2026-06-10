use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("config parse error: {0}")]
    Config(#[from] toml::de::Error),

    #[error("duplicate tool name: `{0}`")]
    DuplicateTool(String),

    #[error("handler not found for tool `{tool}`: {path}")]
    HandlerNotFound { tool: String, path: String },

    #[error("tool not found: `{0}`")]
    ToolNotFound(String),

    #[error("nu execution error: {0}")]
    Execution(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
