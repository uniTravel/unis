#![cfg(feature = "test-utils")]

use std::process::Command;

pub(crate) struct CommandBuilder {
    program: String,
    args: Vec<String>,
}

impl CommandBuilder {
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
        }
    }

    pub fn arg<S: Into<String>>(mut self, arg: S) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for arg in args {
            self.args.push(arg.into());
        }
        self
    }

    pub fn execute(self, description: &str) -> Result<(bool, String, String), String> {
        let output = Command::new(&self.program)
            .args(&self.args)
            .output()
            .map_err(|e| format!("执行 '{} {}' 失败: {}", &self.program, description, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        Ok((success, stdout, stderr))
    }
}
