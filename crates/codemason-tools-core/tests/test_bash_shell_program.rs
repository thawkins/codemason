//! `posix_shell` and `bash_program` name a program that runs a POSIX `-c`
//! script on this host. On Windows there is no `sh` on the default `PATH`, so
//! hooks, verification scripts and `run "!<cmd>"` must spawn the discovered
//! Git Bash instead.

use codemason_tools_core::bash::{bash_program, posix_shell};

fn run(program: std::path::PathBuf) -> String {
    let out = std::process::Command::new(&program)
        .arg("-c")
        .arg("printf '%s' \"$((1 + 1))\"")
        .output()
        .unwrap_or_else(|e| panic!("spawn {}: {e}", program.display()));
    assert!(
        out.status.success(),
        "{} exited with {:?}",
        program.display(),
        out.status
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn test_bash_shell_program_posix_shell_runs_a_posix_script() {
    assert_eq!(run(posix_shell()), "2");
}

#[test]
fn test_bash_shell_program_bash_program_runs_a_posix_script() {
    assert_eq!(run(bash_program()), "2");
}
