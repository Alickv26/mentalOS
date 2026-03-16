use crate::error::{MentalOSError, Result};
use crate::workspace::{ProjectCommands, WorkspaceManager};
use log::info;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRequest {
    pub name: String,
    pub language: Option<String>,
    pub framework: Option<String>,
    pub description: Option<String>,
    pub auto_setup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponse {
    pub success: bool,
    pub path: Option<String>,
    pub message: String,
    pub commands: Option<ProjectCommands>,
}

pub struct ProjectHandler {
    workspace_manager: WorkspaceManager,
    opencode_path: String,
    opencode_args: Vec<String>,
}

impl ProjectHandler {
    pub fn new(workspace_dir: PathBuf, opencode_path: String) -> Self {
        Self {
            workspace_manager: WorkspaceManager::new(workspace_dir),
            opencode_path,
            opencode_args: vec![],
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.opencode_args = args;
        self
    }

    pub fn detect_project_intent(&self, input: &str) -> Option<ProjectRequest> {
        let patterns = Self::intent_patterns();

        for pattern in &patterns {
            if let Some(caps) = pattern.regex.captures(input) {
                let name = caps.get(1).map(|m| m.as_str().trim().to_string())?;
                if Self::looks_like_non_project_pronoun(&name) {
                    continue;
                }

                let language = caps
                    .get(2)
                    .or(caps.get(3))
                    .map(|m| Self::normalize_language(m.as_str()));

                let framework = caps.get(4).map(|m| m.as_str().to_lowercase());

                return Some(ProjectRequest {
                    name,
                    language,
                    framework,
                    description: None,
                    auto_setup: true,
                });
            }
        }

        None
    }

    fn looks_like_non_project_pronoun(name: &str) -> bool {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "this" | "that" | "it" | "project" | "app" | "server" | "tool" | "api"
        )
    }

    fn intent_patterns() -> Vec<IntentPattern> {
        vec![
            // More specific patterns first (with language/framework)
            IntentPattern {
                regex: Regex::new(r"(?i)(?:new|create|build|make)\s+(?:a\s+)?(\w+)\s+(?:app|project|server|tool|api|cli|web)\s+with\s+(\w+)").unwrap(),
            },
            IntentPattern {
                regex: Regex::new(r"(?i)create\s+(?:a\s+)?(\w+)\s+(?:app|project)\s+with\s+(\w+)").unwrap(),
            },
            IntentPattern {
                regex: Regex::new(r"(?i)(?:new|create|build|make)\s+(?:a\s+)?(\w+[-]?\w*)\s+(?:\w+\s+)?(?:app|project|server|tool|api|cli)\s+(?:with|using)\s+(\w+)").unwrap(),
            },
            // Simpler patterns
            IntentPattern {
                regex: Regex::new(r"(?i)create\s+(?:a\s+)?(\w+[-]?\w*)\s+(?:\w+\s+)?(?:app|project|server|tool)?").unwrap(),
            },
            IntentPattern {
                regex: Regex::new(r"(?i)build\s+(?:a\s+)?(\w+[-]?\w*)\s+(?:\w+\s+)?(?:app|project|server|tool)?").unwrap(),
            },
            IntentPattern {
                regex: Regex::new(r"(?i)make\s+(?:a\s+)?(\w+[-]?\w*)\s+(?:\w+\s+)?(?:app|project|server|tool)?").unwrap(),
            },
            IntentPattern {
                regex: Regex::new(r"(?i)scaffold\s+(?:a\s+)?(\w+[-]?\w*)\s+(?:\w+\s+)?(?:app|project|server|tool)?").unwrap(),
            },
        ]
    }

    fn normalize_language(lang: &str) -> String {
        let lang_lower = lang.to_lowercase();
        match lang_lower.as_str() {
            "rust" | "rs" => "rust".to_string(),
            "python" | "py" | "py3" => "python".to_string(),
            "javascript" | "js" | "nodejs" | "node" => "javascript".to_string(),
            "typescript" | "ts" => "typescript".to_string(),
            "go" | "golang" => "go".to_string(),
            "java" => "java".to_string(),
            "c#" | "csharp" | "cs" => "csharp".to_string(),
            "c++" | "cpp" => "cpp".to_string(),
            "c" => "c".to_string(),
            "ruby" | "rb" => "ruby".to_string(),
            "php" => "php".to_string(),
            "swift" => "swift".to_string(),
            "kotlin" | "kt" => "kotlin".to_string(),
            "scala" => "scala".to_string(),
            _ => lang_lower,
        }
    }

    pub fn scaffold_project(
        &self,
        request: &ProjectRequest,
        agent: &str,
    ) -> Result<ProjectResponse> {
        info!(
            "Scaffolding project: {} with agent: {}",
            request.name, agent
        );

        let workspace_path = match self
            .workspace_manager
            .create_workspace(&request.name, agent)
        {
            Ok(path) => path,
            Err(e) => {
                return Ok(ProjectResponse {
                    success: false,
                    path: None,
                    message: format!("Failed to create workspace: {}", e),
                    commands: None,
                });
            }
        };

        let prompt = Self::build_scaffold_prompt(request);

        let output = self.run_opencode(&prompt, &workspace_path)?;

        let commands = self.extract_commands_from_output(&output);

        let metadata_commands = ProjectCommands {
            setup: commands.get("setup").cloned(),
            run: commands.get("run").cloned(),
            test: commands.get("test").cloned(),
        };

        let _ = self.workspace_manager.update_metadata(
            &workspace_path,
            request.language.clone(),
            request.framework.clone(),
            request.description.clone(),
            metadata_commands.clone(),
        );

        let message = if request.auto_setup {
            format!(
                "Created project '{}' at {}. You can run setup with: {}",
                request.name,
                workspace_path.display(),
                commands.get("setup").unwrap_or(&"cargo build".to_string())
            )
        } else {
            format!(
                "Created project '{}' at {}",
                request.name,
                workspace_path.display()
            )
        };

        Ok(ProjectResponse {
            success: true,
            path: Some(workspace_path.to_string_lossy().to_string()),
            message,
            commands: Some(metadata_commands),
        })
    }

    fn build_scaffold_prompt(request: &ProjectRequest) -> String {
        let mut prompt = format!(
            "Create a new {} project called '{}'",
            request.language.as_deref().unwrap_or("application"),
            request.name
        );

        if let Some(ref framework) = request.framework {
            prompt.push_str(&format!(" using {}", framework));
        }

        if let Some(ref description) = request.description {
            prompt.push_str(&format!(". {}", description));
        }

        prompt.push_str(". Set up the basic project structure with:");
        prompt.push_str("\n- Proper package.json/Cargo.toml/etc. configuration");
        prompt.push_str("\n- Basic main entry point");
        prompt.push_str("\n- A setup command that installs dependencies");
        prompt.push_str("\n- A run command to start the project");
        prompt.push_str("\n- A test command if applicable");

        prompt
    }

    fn run_opencode(&self, prompt: &str, workspace: &PathBuf) -> Result<String> {
        let mut command = Command::new(&self.opencode_path);

        if !self.opencode_args.is_empty() {
            command.args(&self.opencode_args);
        } else {
            command.args(["agent"]);
        }

        command.arg("--message").arg(prompt);
        command.current_dir(workspace);

        let output = command
            .output()
            .map_err(|e| MentalOSError::Other(format!("Failed to run opencode: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MentalOSError::Other(format!("OpenCode failed: {}", stderr)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn extract_commands_from_output(
        &self,
        output: &str,
    ) -> std::collections::HashMap<String, String> {
        let mut commands = std::collections::HashMap::new();

        let setup_patterns = [
            r"(?i)setup[:\s]+([^\n]+)",
            r"(?i)install[:\s]+([^\n]+)",
            r"(?i)npm install",
            r"(?i)cargo install",
            r"(?i)pip install",
            r"(?i)go mod download",
        ];

        let run_patterns = [
            r"(?i)run[:\s]+([^\n]+)",
            r"(?i)start[:\s]+([^\n]+)",
            r"(?i)npm start",
            r"(?i)cargo run",
            r"(?i)python.*\.py",
        ];

        let test_patterns = [
            r"(?i)test[:\s]+([^\n]+)",
            r"(?i)npm test",
            r"(?i)cargo test",
            r"(?i)pytest",
        ];

        for pattern in &setup_patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(output) {
                    if let Some(m) = cap.get(1) {
                        commands.insert("setup".to_string(), m.as_str().trim().to_string());
                        break;
                    }
                }
            }
        }

        for pattern in &run_patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(output) {
                    if let Some(m) = cap.get(1) {
                        commands.insert("run".to_string(), m.as_str().trim().to_string());
                        break;
                    }
                }
            }
        }

        for pattern in &test_patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(output) {
                    if let Some(m) = cap.get(1) {
                        commands.insert("test".to_string(), m.as_str().trim().to_string());
                        break;
                    }
                }
            }
        }

        commands
    }

    pub fn resolve_project_command(&self, workspace: &Path, command_type: &str) -> Result<String> {
        let metadata = self.workspace_manager.load_metadata(workspace)?;
        let command = Self::pick_project_command(metadata, command_type)
            .ok_or_else(|| MentalOSError::Other(format!("No {} command found", command_type)))?;
        Ok(command)
    }

    pub fn run_project_command(&self, workspace: &PathBuf, command_type: &str) -> Result<String> {
        let command = self.resolve_project_command(workspace, command_type)?;
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(workspace)
            .output()
            .map_err(|e| MentalOSError::Io(e))?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn pick_project_command(
        metadata: Option<crate::workspace::ProjectMetadata>,
        command_type: &str,
    ) -> Option<String> {
        match command_type {
            "setup" => metadata.and_then(|m| m.commands.setup),
            "run" => metadata.and_then(|m| m.commands.run),
            "test" => metadata.and_then(|m| m.commands.test),
            _ => None,
        }
    }
}

struct IntentPattern {
    regex: Regex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_handler() -> ProjectHandler {
        ProjectHandler::new(PathBuf::from("/tmp"), "opencode".to_string())
    }

    #[test]
    fn detects_create_intent() {
        let handler = test_handler();
        let request = handler.detect_project_intent("Create a Rust web server");
        assert!(request.is_some());
        let req = request.unwrap();
        assert_eq!(req.name, "Rust");
    }

    #[test]
    fn does_not_detect_pronoun_as_project_name() {
        let handler = test_handler();
        let request = handler.detect_project_intent("build this");
        assert!(request.is_none());
    }

    #[test]
    fn detects_build_intent() {
        let handler = test_handler();
        let request = handler.detect_project_intent("Build a Python web scraper");
        assert!(request.is_some());
        let req = request.unwrap();
        assert_eq!(req.name, "Python");
    }

    #[test]
    fn detects_with_language() {
        let handler = test_handler();
        let request = handler.detect_project_intent("Create a new React app with typescript");
        assert!(request.is_some());
        let req = request.unwrap();
        assert_eq!(req.language, Some("typescript".to_string()));
    }

    #[test]
    fn normalizes_language() {
        assert_eq!(ProjectHandler::normalize_language("rs"), "rust");
        assert_eq!(ProjectHandler::normalize_language("python"), "python");
        assert_eq!(ProjectHandler::normalize_language("js"), "javascript");
        assert_eq!(ProjectHandler::normalize_language("go"), "go");
    }
}
