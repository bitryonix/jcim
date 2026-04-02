use super::*;

/// Diagnostic output returned by the external card-management tools.
pub(super) struct CardToolOutput {
    /// Combined stdout/stderr lines emitted by the tool invocation.
    pub(super) output_lines: Vec<String>,
}

/// Return the bundled JCIM card-helper JAR path.
pub(crate) fn helper_jar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/jcim_card_helper/jcim-card-helper.jar")
}

/// Return the bundled GPPro JAR path.
pub(crate) fn gppro_jar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/gppro/gp.jar")
}

/// Build the optional reader argument list used by helper-tool commands.
pub(super) fn reader_arg_list(reader_name: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(reader_name) = reader_name {
        args.push("-r".to_string());
        args.push(reader_name.to_string());
    }
    args
}

/// Run the JCIM helper tool without additional GP secure-channel environment variables.
pub(super) async fn run_card_helper(
    user_config: &UserConfig,
    action: &str,
    reader_name: Option<&str>,
    extra_args: &[String],
) -> Result<String> {
    run_card_helper_with_env(user_config, action, reader_name, extra_args, None).await
}

/// Run the JCIM helper tool with optional GP keyset environment injected into the process.
pub(super) async fn run_card_helper_with_env(
    user_config: &UserConfig,
    action: &str,
    reader_name: Option<&str>,
    extra_args: &[String],
    gp_keyset: Option<&ResolvedGpKeyset>,
) -> Result<String> {
    let mut command = Command::new(&user_config.java_bin);
    command
        .arg("-cp")
        .arg(helper_classpath())
        .arg("jcim.cardhelper.Main")
        .arg(action)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(keyset) = gp_keyset {
        keyset.apply_helper_env(&mut command);
    }
    if let Some(reader_name) = reader_name {
        command.arg("--reader").arg(reader_name);
    }
    command.args(extra_args);
    run_command_to_string(command, format!("card helper {action}")).await
}

/// Build the Java classpath used by the JCIM helper tool entrypoint.
fn helper_classpath() -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    [helper_jar_path(), gppro_jar_path()]
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Run GPPro and return its combined output lines.
pub(super) async fn run_gppro(user_config: &UserConfig, args: &[String]) -> Result<CardToolOutput> {
    let mut command = Command::new(&user_config.java_bin);
    command
        .arg("-jar")
        .arg(gppro_jar_path())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_command_to_string(command, "GPPro command".to_string()).await?;
    Ok(CardToolOutput {
        output_lines: output.lines().map(|line| line.to_string()).collect(),
    })
}

/// Execute one host command, merge stdout/stderr, and surface non-zero exits as JCIM errors.
async fn run_command_to_string(mut command: Command, description: String) -> Result<String> {
    let output = command
        .output()
        .await
        .map_err(|error| JcimError::Unsupported(format!("unable to run {description}: {error}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.trim().is_empty() {
        stdout.to_string()
    } else if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    if output.status.success() {
        Ok(combined)
    } else {
        Err(JcimError::Unsupported(format!(
            "{description} failed with status {}: {}",
            output.status,
            combined.trim()
        )))
    }
}

#[cfg(test)]
pub(super) fn host_tool_temp_root(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp").join(format!("jcim-host-tool-{label}-{unique:x}"))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(super) fn write_fake_java(root: &Path, script_body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(root).expect("create fake java root");
    let path = root.join("fake-java");
    std::fs::write(&path, script_body).expect("write fake java script");
    let mut permissions = std::fs::metadata(&path)
        .expect("fake java metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("mark fake java executable");
    path
}

#[cfg(test)]
pub(super) fn fake_java_user_config(java_bin: &Path) -> UserConfig {
    UserConfig {
        java_bin: java_bin.display().to_string(),
        ..UserConfig::default()
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::card::ResolvedGpKeyset;

    #[test]
    fn reader_arg_list_adds_optional_reader_flag() {
        assert_eq!(reader_arg_list(None), Vec::<String>::new());
        assert_eq!(
            reader_arg_list(Some("Reader 0")),
            vec!["-r".to_string(), "Reader 0".to_string()]
        );
    }

    #[tokio::test]
    async fn run_card_helper_builds_expected_command_and_merges_output() {
        let root = host_tool_temp_root("helper-merge");
        let java_bin = write_fake_java(
            &root,
            r#"#!/bin/sh
set -eu
DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" > "$DIR/args.log"
{
  printf 'JCIM_GP_MODE=%s\n' "${JCIM_GP_MODE-}"
  printf 'JCIM_GP_ENC=%s\n' "${JCIM_GP_ENC-}"
  printf 'JCIM_GP_MAC=%s\n' "${JCIM_GP_MAC-}"
  printf 'JCIM_GP_DEK=%s\n' "${JCIM_GP_DEK-}"
} > "$DIR/env.log"
printf 'stdout-line\n'
printf 'stderr-line\n' >&2
"#,
        );
        let user_config = fake_java_user_config(&java_bin);
        let keyset = ResolvedGpKeyset::resolve(Some("__test__")).expect("test keyset");

        let output = run_card_helper_with_env(
            &user_config,
            "merge",
            Some("Reader 0"),
            &["--flag".to_string(), "value".to_string()],
            Some(&keyset),
        )
        .await
        .expect("run helper");

        assert_eq!(output, "stdout-line\nstderr-line\n");
        assert_eq!(
            read_lines(&root.join("args.log")),
            vec![
                "-cp".to_string(),
                helper_classpath(),
                "jcim.cardhelper.Main".to_string(),
                "merge".to_string(),
                "--reader".to_string(),
                "Reader 0".to_string(),
                "--flag".to_string(),
                "value".to_string(),
            ]
        );
        let env = read_lines(&root.join("env.log"));
        assert!(env.contains(&"JCIM_GP_MODE=scp03".to_string()));
        assert!(env.iter().any(|line| line.starts_with("JCIM_GP_ENC=4041")));
        assert!(env.iter().any(|line| line.starts_with("JCIM_GP_MAC=5051")));
        assert!(env.iter().any(|line| line.starts_with("JCIM_GP_DEK=6061")));

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn run_gppro_reports_failures_and_uses_expected_jar_path() {
        let root = host_tool_temp_root("gppro-fail");
        let java_bin = write_fake_java(
            &root,
            r#"#!/bin/sh
set -eu
DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" > "$DIR/args.log"
printf 'gppro failed\n' >&2
exit 7
"#,
        );
        let user_config = fake_java_user_config(&java_bin);

        let error = match run_gppro(&user_config, &["-l".to_string()]).await {
            Ok(_) => panic!("gppro failure should propagate"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("GPPro command failed with status")
        );
        assert!(error.to_string().contains("gppro failed"));
        let args = read_lines(&root.join("args.log"));
        assert_eq!(args[0], "-jar");
        assert_eq!(args[1], gppro_jar_path().display().to_string());
        assert_eq!(args[2], "-l");

        let _ = std::fs::remove_dir_all(root);
    }

    fn read_lines(path: &Path) -> Vec<String> {
        std::fs::read_to_string(path)
            .expect("read log")
            .lines()
            .map(str::to_string)
            .collect()
    }
}
