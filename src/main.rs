const ABC_INPUT_SOURCE: &str = "com.apple.keylayout.ABC";
const AGENT_NAME: &str = "codex";

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct State {
    last_pane_id: Option<String>,
    last_agent: Option<String>,
    new_pane_ids: std::collections::BTreeSet<String>,
}

fn pane_id_from(payload: &serde_json::Value) -> Option<&str> {
    let event_data = payload.get("data").unwrap_or(payload);
    event_data
        .get("pane_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            event_data
                .get("pane")
                .and_then(|pane| pane.get("pane_id"))
                .and_then(serde_json::Value::as_str)
        })
}

fn should_select_abc(
    previous_agent: Option<&str>,
    current_agent: Option<&str>,
    is_new_pane: bool,
) -> bool {
    is_new_pane || (previous_agent == Some(AGENT_NAME) && current_agent.is_none())
}

fn current_pane() -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
    let herdr = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_owned());
    let output = std::process::Command::new(herdr)
        .args(["pane", "current"])
        .env_remove("HERDR_PANE_ID")
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let payload: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    Ok(payload
        .get("result")
        .and_then(|result| result.get("pane"))
        .cloned())
}

fn select_abc() -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("macism")
        .arg(ABC_INPUT_SOURCE)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("macism exited with {status}").into())
    }
}

fn load_state(path: &std::path::Path) -> Result<State, Box<dyn std::error::Error>> {
    match std::fs::read(path) {
        Ok(contents) => Ok(serde_json::from_slice(&contents)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(error) => Err(error.into()),
    }
}

fn save_state(path: &std::path::Path, state: &State) -> Result<(), Box<dyn std::error::Error>> {
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temporary, serde_json::to_vec(state)?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn update_state(
    command: &str,
    payload: &serde_json::Value,
    state: &mut State,
) -> Result<(), Box<dyn std::error::Error>> {
    let pane_id = pane_id_from(payload).map(str::to_owned);

    match command {
        "pane-created" => {
            if let Some(pane_id) = pane_id {
                state.new_pane_ids.insert(pane_id);
            }
            return Ok(());
        }
        "pane-closed" => {
            if let Some(pane_id) = pane_id {
                state.new_pane_ids.remove(&pane_id);
                if state.last_pane_id.as_deref() == Some(&pane_id) {
                    state.last_pane_id = None;
                    state.last_agent = None;
                }
            }
            return Ok(());
        }
        "pane-agent-detected" => {
            if pane_id.as_deref() == state.last_pane_id.as_deref() {
                state.last_agent = payload
                    .get("data")
                    .unwrap_or(payload)
                    .get("agent")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
            }
            return Ok(());
        }
        "pane-focused" => {}
        _ => return Err(format!("unknown command: {command}").into()),
    }

    let Some(pane) = current_pane()? else {
        return Ok(());
    };
    let Some(focused_pane_id) = pane.get("pane_id").and_then(serde_json::Value::as_str) else {
        return Ok(());
    };
    if pane_id.as_deref().is_some_and(|id| id != focused_pane_id) {
        return Ok(());
    }
    let current_agent = pane.get("agent").and_then(serde_json::Value::as_str);
    let is_new_pane = state.new_pane_ids.contains(focused_pane_id);
    if should_select_abc(state.last_agent.as_deref(), current_agent, is_new_pane) {
        select_abc()?;
    }

    state.new_pane_ids.remove(focused_pane_id);
    state.last_pane_id = Some(focused_pane_id.to_owned());
    state.last_agent = current_agent.map(str::to_owned);
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let command = std::env::args()
        .nth(1)
        .ok_or("usage: herdr-input-source-router <command>")?;
    let state_dir = std::env::var_os("HERDR_PLUGIN_STATE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&state_dir)?;
    let state_path = state_dir.join("state.json");

    if command == "status" {
        println!(
            "{}",
            serde_json::to_string_pretty(&load_state(&state_path)?)?
        );
        return Ok(());
    }

    let event_payload = std::env::var("HERDR_PLUGIN_EVENT_JSON")
        .ok()
        .map(|value| serde_json::from_str(&value))
        .transpose()?
        .unwrap_or_else(|| serde_json::json!({}));
    let lock_path = state_dir.join("state.lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(lock_path)?;
    {
        use fs2::FileExt;
        lock_file.lock_exclusive()?;
    }
    let mut state = load_state(&state_path)?;
    update_state(&command, &event_payload, &mut state)?;
    save_state(&state_path, &state)?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_focused_pane_id_from_event_envelope() {
        let payload = serde_json::json!({
            "event": "pane.focused",
            "data": {"pane_id": "w1:p2"}
        });
        assert_eq!(crate::pane_id_from(&payload), Some("w1:p2"));
    }

    #[test]
    fn reads_created_pane_id_from_event_envelope() {
        let payload = serde_json::json!({
            "event": "pane.created",
            "data": {"pane": {"pane_id": "w1:p3"}}
        });
        assert_eq!(crate::pane_id_from(&payload), Some("w1:p3"));
    }

    #[test]
    fn new_pane_selects_abc() {
        assert!(crate::should_select_abc(Some("codex"), Some("codex"), true));
    }

    #[test]
    fn codex_to_shell_selects_abc() {
        assert!(crate::should_select_abc(Some("codex"), None, false));
    }

    #[test]
    fn codex_to_codex_keeps_current_input_source() {
        assert!(!crate::should_select_abc(
            Some("codex"),
            Some("codex"),
            false
        ));
    }

    #[test]
    fn shell_to_codex_keeps_current_input_source() {
        assert!(!crate::should_select_abc(None, Some("codex"), false));
    }
}
