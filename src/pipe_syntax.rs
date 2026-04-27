//! Pipe syntax configuration.

use std::str::FromStr;

/// Environment variable used to select the accepted dplyr pipe syntax.
pub const PIPE_SYNTAX_ENV_VAR: &str = "DPLYR_PIPE_SYNTAX";

/// Supported pipe syntaxes for libdplyr's dplyr-subset grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PipeSyntax {
    /// magrittr pipe syntax: `%>%`
    #[default]
    Magrittr,
    /// base R native pipe syntax: `|>`
    Native,
}

impl PipeSyntax {
    /// Returns the operator spelling accepted by this mode.
    pub const fn operator(self) -> &'static str {
        match self {
            Self::Magrittr => "%>%",
            Self::Native => "|>",
        }
    }

    /// Returns the other pipe syntax.
    pub const fn opposite(self) -> Self {
        match self {
            Self::Magrittr => Self::Native,
            Self::Native => Self::Magrittr,
        }
    }

    /// Error message used when this syntax is seen while disabled.
    pub const fn disabled_message(self) -> &'static str {
        match self {
            Self::Magrittr => "Magrittr pipe is not enabled",
            Self::Native => "Native pipe is not enabled",
        }
    }

    /// Canonical configuration value.
    pub const fn config_value(self) -> &'static str {
        match self {
            Self::Magrittr => "magrittr",
            Self::Native => "native",
        }
    }

    /// Rust enum variant name used in public guidance.
    pub const fn rust_variant(self) -> &'static str {
        match self {
            Self::Magrittr => "Magrittr",
            Self::Native => "Native",
        }
    }

    /// Guidance for enabling this syntax through supported configuration paths.
    pub fn disabled_suggestion(self) -> String {
        format!(
            "Set {PIPE_SYNTAX_ENV_VAR}={} before process start or use an explicit pipe syntax API with PipeSyntax::{}",
            self.config_value(),
            self.rust_variant()
        )
    }

    /// Error text used when this syntax is seen while disabled.
    pub fn disabled_error(self) -> String {
        format!(
            "{}. {}",
            self.disabled_message(),
            self.disabled_suggestion()
        )
    }

    /// Reads pipe syntax from `DPLYR_PIPE_SYNTAX`.
    pub fn from_env() -> Result<Option<Self>, String> {
        match std::env::var(PIPE_SYNTAX_ENV_VAR) {
            Ok(value) => value.parse().map(Some),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(format!(
                "{PIPE_SYNTAX_ENV_VAR} must be valid Unicode with value 'magrittr' or 'native'"
            )),
        }
    }

    /// Reads pipe syntax from `DPLYR_PIPE_SYNTAX`, defaulting to Magrittr.
    pub fn from_env_or_default() -> Result<Self, String> {
        Ok(Self::from_env()?.unwrap_or_default())
    }
}

/// Returns pipe syntax configuration guidance when an error mentions a disabled pipe.
pub(crate) fn disabled_pipe_suggestion_for_error(message: &str) -> Option<String> {
    if message.contains(PipeSyntax::Native.disabled_message()) {
        Some(PipeSyntax::Native.disabled_suggestion())
    } else if message.contains(PipeSyntax::Magrittr.disabled_message()) {
        Some(PipeSyntax::Magrittr.disabled_suggestion())
    } else {
        None
    }
}

impl FromStr for PipeSyntax {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "magrittr" | "%>%" => Ok(Self::Magrittr),
            "native" | "|>" => Ok(Self::Native),
            other => Err(format!(
                "Invalid pipe syntax '{other}'. Expected 'magrittr' or 'native'"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        _guard: MutexGuard<'static, ()>,
        original: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn capture() -> Self {
            let guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
            let original = std::env::var_os(PIPE_SYNTAX_ENV_VAR);
            Self {
                _guard: guard,
                original,
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => std::env::set_var(PIPE_SYNTAX_ENV_VAR, value),
                None => std::env::remove_var(PIPE_SYNTAX_ENV_VAR),
            }
        }
    }

    #[test]
    fn environment_configures_env_backed_default() {
        let _restore = EnvRestore::capture();

        std::env::set_var(PIPE_SYNTAX_ENV_VAR, PipeSyntax::Native.config_value());

        assert_eq!(PipeSyntax::from_env_or_default(), Ok(PipeSyntax::Native));
    }

    #[test]
    fn disabled_pipe_suggestion_mentions_env_and_explicit_api() {
        assert_eq!(
            PipeSyntax::Native.disabled_suggestion(),
            "Set DPLYR_PIPE_SYNTAX=native before process start or use an explicit pipe syntax API with PipeSyntax::Native"
        );
    }
}
