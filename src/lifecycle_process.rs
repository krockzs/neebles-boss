use std::collections::BTreeMap;
use std::path::PathBuf;

use async_process::Command;

/*
 * Lifecycle direct process execution primitive.
 *
 * Technical responsibility:
 * Execute one external process directly through Rust.
 *
 * This layer does not use a shell.
 * It does not interpret Lifecycle artillery, objective, munition,
 * tactics, intelligence or module semantics.
 *
 * It only receives an explicit technical ProcessRequest.
 */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRequest {
    pub executable: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
}

impl ProcessRequest {
    pub fn new(executable: impl Into<String>) -> Result<Self, String> {
        let executable = executable.into();

        if executable.trim().is_empty() {
            return Err("process executable cannot be empty".to_string());
        }

        Ok(Self {
            executable,
            arguments: Vec::new(),
            working_directory: None,
            environment: BTreeMap::new(),
        })
    }

    pub fn argument(mut self, argument: impl Into<String>) -> Self {
        self.arguments.push(argument.into());

        self
    }

    pub fn working_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(directory.into());

        self
    }

    pub fn environment(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, String> {
        let key = key.into();

        if key.trim().is_empty() {
            return Err("process environment key cannot be empty".to_string());
        }

        self.environment.insert(key, value.into());

        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessResult {
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }
}

pub async fn execute(request: ProcessRequest) -> Result<ProcessResult, String> {
    let mut command = Command::new(&request.executable);

    command.args(&request.arguments);

    if let Some(directory) = &request.working_directory {
        command.current_dir(directory);
    }

    for (key, value) in &request.environment {
        command.env(key, value);
    }

    let output = command.output().await.map_err(|error| {
        format!(
            "could not execute process '{}': {error}",
            request.executable
        )
    })?;

    Ok(ProcessResult {
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_empty_executable() {
        let error = ProcessRequest::new("   ").unwrap_err();

        assert!(error.contains("executable cannot be empty"));
    }

    #[test]
    fn request_accepts_arbitrary_executable() {
        let request = ProcessRequest::new("/future/arbitrary/executable").unwrap();

        assert_eq!(request.executable, "/future/arbitrary/executable");
    }

    #[test]
    fn arguments_remain_literal_data() {
        let request = ProcessRequest::new("synthetic")
            .unwrap()
            .argument("hello;world")
            .argument("one && two");

        assert_eq!(
            request.arguments,
            vec!["hello;world".to_string(), "one && two".to_string(),]
        );
    }

    #[test]
    fn working_directory_is_explicit() {
        let request = ProcessRequest::new("synthetic")
            .unwrap()
            .working_directory("/future/work");

        assert_eq!(
            request.working_directory,
            Some(PathBuf::from("/future/work"))
        );
    }

    #[test]
    fn environment_is_explicit() {
        let request = ProcessRequest::new("synthetic")
            .unwrap()
            .environment("NEEBLES_SYNTHETIC", "opaque-value")
            .unwrap();

        assert_eq!(
            request.environment.get("NEEBLES_SYNTHETIC"),
            Some(&"opaque-value".to_string())
        );
    }

    #[test]
    fn environment_rejects_empty_key() {
        let error = ProcessRequest::new("synthetic")
            .unwrap()
            .environment("   ", "value")
            .unwrap_err();

        assert!(error.contains("environment key cannot be empty"));
    }

    #[test]
    fn executes_current_rust_test_binary_directly() {
        let executable = std::env::current_exe().unwrap();

        let request = ProcessRequest::new(executable.to_string_lossy().into_owned())
            .unwrap()
            .argument("--list");

        let result = futures_lite::future::block_on(execute(request)).unwrap();

        assert!(result.succeeded());

        assert!(!result.stdout.is_empty());
    }

    #[test]
    fn unknown_executable_returns_error() {
        let request = ProcessRequest::new("/definitely/not/a/real/neebles/executable").unwrap();

        let error = futures_lite::future::block_on(execute(request)).unwrap_err();

        assert!(error.contains("could not execute process"));
    }

    #[test]
    fn nonzero_exit_is_result_not_transport_error() {
        let executable = std::env::current_exe().unwrap();

        let request = ProcessRequest::new(executable.to_string_lossy().into_owned())
            .unwrap()
            .argument("--definitely-invalid-test-argument");

        let result = futures_lite::future::block_on(execute(request)).unwrap();

        assert!(!result.succeeded());
    }
}
