use super::*;

/// Diagnostic output returned by the external card-management tools.
pub(super) struct CardToolOutput {
    pub(super) output_lines: Vec<String>,
}

pub(crate) fn helper_jar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/jcim_card_helper/jcim-card-helper.jar")
}

pub(crate) fn gppro_jar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/gppro/gp.jar")
}

pub(super) fn reader_arg_list(reader_name: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(reader_name) = reader_name {
        args.push("-r".to_string());
        args.push(reader_name.to_string());
    }
    args
}

pub(super) async fn run_card_helper(
    user_config: &UserConfig,
    action: &str,
    reader_name: Option<&str>,
    extra_args: &[String],
) -> Result<String> {
    run_card_helper_with_env(user_config, action, reader_name, extra_args, None).await
}

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

fn helper_classpath() -> String {
    let separator = if cfg!(windows) { ";" } else { ":" };
    [helper_jar_path(), gppro_jar_path()]
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

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
