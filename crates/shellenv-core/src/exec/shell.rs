use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::str::FromStr;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

use super::ExecError;

/// Supported non-interactive execution shells.
///
/// This names an execution mode; it does not imply the executable is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Pwsh,
    Cmd,
}

/// Structured result of a non-interactive shell invocation.
///
/// `stdout` and `stderr` are strict UTF-8. `duration_ms` is measured with
/// [`std::time::Instant`], not wall-clock time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

impl Shell {
    /// Executable basename resolved via `PATH` at spawn time.
    pub fn executable(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::Pwsh => "pwsh",
            Shell::Cmd => "cmd",
        }
    }

    /// Arguments that run `command` without loading interactive rc/profile files.
    pub fn noninteractive_command(self, command: &str) -> Vec<String> {
        match self {
            Shell::Bash => vec![
                "--noprofile".into(),
                "--norc".into(),
                "-c".into(),
                command.into(),
            ],
            Shell::Zsh => vec!["-f".into(), "-c".into(), command.into()],
            Shell::Fish => vec!["-c".into(), command.into()],
            Shell::Pwsh => vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                command.into(),
            ],
            Shell::Cmd => vec!["/C".into(), command.into()],
        }
    }
}

impl FromStr for Shell {
    type Err = ExecError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "bash" => Ok(Shell::Bash),
            "zsh" => Ok(Shell::Zsh),
            "fish" => Ok(Shell::Fish),
            "pwsh" => Ok(Shell::Pwsh),
            "cmd" => Ok(Shell::Cmd),
            _ => Err(ExecError::InvalidShell {
                found: s.to_string(),
            }),
        }
    }
}

/// Run `command` in `shell` with a fully specified environment.
///
/// `cwd` inherits the current directory when `None`. When `timeout_ms` elapses,
/// the child process (and, on Unix, its process group) is killed and
/// [`ExecError::Timeout`] is returned.
///
/// The `run` API executes a **shell command string**, not an argv array.
pub fn run_in_shell(
    shell: Shell,
    command: &str,
    cwd: Option<&Path>,
    env: &BTreeMap<String, String>,
    timeout_ms: Option<u64>,
) -> Result<ExecResult, ExecError> {
    if command.is_empty() {
        return Err(ExecError::MissingCommand);
    }

    let mut cmd = Command::new(shell.executable());
    cmd.args(shell.noninteractive_command(command))
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let start = Instant::now();
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(ExecError::ShellNotFound(shell.executable().to_string()));
        }
        Err(err) => {
            return Err(ExecError::SpawnFailed {
                shell: shell.executable().to_string(),
                source: err,
            });
        }
    };

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("failed to capture stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("failed to capture stderr"))?;

    let stdout_h = spawn_reader(stdout);
    let stderr_h = spawn_reader(stderr);

    let wait_result = wait_for_exit(&mut child, timeout_ms);
    let stdout_bytes = join_reader(stdout_h);
    let stderr_bytes = join_reader(stderr_h);

    match wait_result {
        Ok(status) => Ok(ExecResult {
            exit_code: exit_code(status),
            stdout: String::from_utf8(stdout_bytes?)?,
            stderr: String::from_utf8(stderr_bytes?)?,
            duration_ms: duration_ms(start),
        }),
        Err(err) => {
            let _ = stdout_bytes;
            let _ = stderr_bytes;
            Err(err)
        }
    }
}

fn spawn_reader<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        pipe.read_to_end(&mut buf)?;
        Ok(buf)
    })
}

fn join_reader(handle: JoinHandle<io::Result<Vec<u8>>>) -> Result<Vec<u8>, ExecError> {
    handle
        .join()
        .map_err(|_| ExecError::OutputCapture)?
        .map_err(ExecError::Io)
}

fn wait_for_exit(child: &mut Child, timeout_ms: Option<u64>) -> Result<ExitStatus, ExecError> {
    match timeout_ms {
        None => Ok(child.wait()?),
        Some(ms) => match child.wait_timeout(Duration::from_millis(ms))? {
            Some(status) => Ok(status),
            None => {
                kill_tree(child);
                Err(ExecError::Timeout)
            }
        },
    }
}

fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // Negative pid sends SIGKILL to the process group created by process_group(0).
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    1
}

fn duration_ms(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    fn parent_env() -> BTreeMap<String, String> {
        std::env::vars().collect()
    }

    fn bash_available() -> bool {
        std::process::Command::new("bash")
            .args(["--noprofile", "--norc", "-c", "exit 0"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn leftover_sleep_running() -> bool {
        let Ok(output) = std::process::Command::new("ps")
            .args(["-ax", "-o", "args="])
            .output()
        else {
            return false;
        };
        String::from_utf8_lossy(&output.stdout).lines().any(|line| {
            line.contains("sleep 8.765") && !line.contains("ps ") && !line.contains("pgrep")
        })
    }

    #[test]
    fn parses_supported_shell_names() {
        assert_eq!("bash".parse::<Shell>().unwrap(), Shell::Bash);
        assert_eq!("zsh".parse::<Shell>().unwrap(), Shell::Zsh);
        assert_eq!("fish".parse::<Shell>().unwrap(), Shell::Fish);
        assert_eq!("pwsh".parse::<Shell>().unwrap(), Shell::Pwsh);
        assert_eq!("cmd".parse::<Shell>().unwrap(), Shell::Cmd);
        assert_eq!("BASH".parse::<Shell>().unwrap(), Shell::Bash);
    }

    #[test]
    fn rejects_unknown_shell_names() {
        let err = "definitely-not-a-shell".parse::<Shell>().unwrap_err();
        let message = err.to_string();
        assert!(message.contains("definitely-not-a-shell"));
        assert!(message.contains("bash"));
        assert!(message.contains("zsh"));
        assert!(message.contains("fish"));
        assert!(message.contains("pwsh"));
        assert!(message.contains("cmd"));
    }

    #[test]
    fn noninteractive_arguments_avoid_rc_files() {
        assert_eq!(
            Shell::Bash.noninteractive_command("echo hi"),
            vec!["--noprofile", "--norc", "-c", "echo hi"]
        );
        assert_eq!(
            Shell::Zsh.noninteractive_command("echo hi"),
            vec!["-f", "-c", "echo hi"]
        );
        assert_eq!(
            Shell::Fish.noninteractive_command("echo hi"),
            vec!["-c", "echo hi"]
        );
        assert_eq!(
            Shell::Pwsh.noninteractive_command("Write-Output hi"),
            vec![
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Write-Output hi"
            ]
        );
        assert_eq!(
            Shell::Cmd.noninteractive_command("echo hi"),
            vec!["/C", "echo hi"]
        );
    }

    #[test]
    fn executables_are_path_basenames() {
        assert_eq!(Shell::Bash.executable(), "bash");
        assert_eq!(Shell::Zsh.executable(), "zsh");
        assert_eq!(Shell::Fish.executable(), "fish");
        assert_eq!(Shell::Pwsh.executable(), "pwsh");
        assert_eq!(Shell::Cmd.executable(), "cmd");
    }

    #[test]
    fn successful_command_captures_stdout() {
        if !bash_available() {
            return;
        }
        let result = run_in_shell(Shell::Bash, "echo hello", None, &parent_env(), None).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello\n");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn captures_stderr_separately_from_stdout() {
        if !bash_available() {
            return;
        }
        let result = run_in_shell(
            Shell::Bash,
            "echo out; echo err >&2",
            None,
            &parent_env(),
            None,
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "out\n");
        assert_eq!(result.stderr, "err\n");
    }

    #[test]
    fn environment_variables_are_visible_to_the_command() {
        if !bash_available() {
            return;
        }
        let mut env = parent_env();
        env.insert("MY_VAR".into(), "from_profile".into());
        let result = run_in_shell(Shell::Bash, "echo MY_VAR=$MY_VAR", None, &env, None).unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "MY_VAR=from_profile\n");
    }

    #[test]
    fn non_zero_exit_code_is_propagated() {
        if !bash_available() {
            return;
        }
        let result = run_in_shell(Shell::Bash, "false", None, &parent_env(), None).unwrap();
        assert_eq!(result.exit_code, 1);

        let result = run_in_shell(Shell::Bash, "exit 7", None, &parent_env(), None).unwrap();
        assert_eq!(result.exit_code, 7);
    }

    #[test]
    fn cwd_changes_the_working_directory() {
        if !bash_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let result =
            run_in_shell(Shell::Bash, "pwd", Some(dir.path()), &parent_env(), None).unwrap();
        assert_eq!(result.exit_code, 0);
        let expected = dir.path().canonicalize().unwrap();
        let actual = std::path::PathBuf::from(result.stdout.trim())
            .canonicalize()
            .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn missing_command_is_an_error() {
        let err = run_in_shell(Shell::Bash, "", None, &parent_env(), None).unwrap_err();
        assert!(matches!(err, ExecError::MissingCommand));
    }

    #[test]
    fn unknown_shell_executable_is_a_not_found_error() {
        if cfg!(windows) {
            return;
        }
        let err = run_in_shell(Shell::Cmd, "echo hi", None, &BTreeMap::new(), None).unwrap_err();
        assert!(matches!(err, ExecError::ShellNotFound(_)));
    }

    #[test]
    fn timeout_kills_the_child_and_does_not_leave_it_running() {
        if !bash_available() {
            return;
        }
        let start = Instant::now();
        let err =
            run_in_shell(Shell::Bash, "sleep 8.765", None, &parent_env(), Some(150)).unwrap_err();
        assert!(matches!(err, ExecError::Timeout));
        assert!(start.elapsed() < Duration::from_secs(2));

        std::thread::sleep(Duration::from_millis(200));
        assert!(
            !leftover_sleep_running(),
            "timed-out sleep 8.765 is still running"
        );
    }

    #[test]
    fn duration_is_measured_for_successful_commands() {
        if !bash_available() {
            return;
        }
        let result = run_in_shell(Shell::Bash, "true", None, &parent_env(), None).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.duration_ms < 5_000);
    }

    #[test]
    fn large_stdout_does_not_deadlock() {
        if !bash_available() {
            return;
        }
        let result = run_in_shell(
            Shell::Bash,
            "dd if=/dev/zero bs=1024 count=128 2>/dev/null | tr '\\0' 'a'",
            None,
            &parent_env(),
            Some(10_000),
        )
        .unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.len(), 128 * 1024);
        assert!(result.stdout.bytes().all(|b| b == b'a'));
    }
}
