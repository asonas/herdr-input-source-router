const DEFAULT_INPUT_SOURCE: &str = "com.apple.keylayout.ABC";

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
struct State {
    #[serde(default, alias = "last_pane_id")]
    last_focused_pane_id: Option<String>,
    #[serde(default)]
    panes: std::collections::BTreeMap<String, PaneMemory>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct PaneMemory {
    input_source_id: String,
}

fn event_data(payload: &serde_json::Value) -> &serde_json::Value {
    payload.get("data").unwrap_or(payload)
}

fn pane_id_from(payload: &serde_json::Value) -> Option<&str> {
    crate::event_data(payload)
        .get("pane_id")
        .and_then(serde_json::Value::as_str)
}

fn remember_focus(
    state: &mut State,
    focused_pane_id: &str,
    observed_input_source: &str,
) -> Option<String> {
    let Some(previous_pane_id) = state.last_focused_pane_id.clone() else {
        state.panes.insert(
            focused_pane_id.to_owned(),
            PaneMemory {
                input_source_id: observed_input_source.to_owned(),
            },
        );
        state.last_focused_pane_id = Some(focused_pane_id.to_owned());
        return None;
    };

    if previous_pane_id == focused_pane_id {
        state
            .panes
            .entry(focused_pane_id.to_owned())
            .or_insert_with(|| PaneMemory {
                input_source_id: observed_input_source.to_owned(),
            });
        return None;
    }

    state.panes.insert(
        previous_pane_id,
        PaneMemory {
            input_source_id: observed_input_source.to_owned(),
        },
    );
    let target = state
        .panes
        .entry(focused_pane_id.to_owned())
        .or_insert_with(|| PaneMemory {
            input_source_id: DEFAULT_INPUT_SOURCE.to_owned(),
        })
        .input_source_id
        .clone();
    state.last_focused_pane_id = Some(focused_pane_id.to_owned());

    (target != observed_input_source).then_some(target)
}

fn current_pane_id() -> Result<Option<String>, Box<dyn std::error::Error>> {
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
        .and_then(|pane| pane.get("pane_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

fn current_input_source() -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("macism").output()?;
    if !output.status.success() {
        return Err(format!("macism exited with {}", output.status).into());
    }
    let input_source = String::from_utf8(output.stdout)?.trim().to_owned();
    if input_source.is_empty() {
        return Err("macism returned an empty input source".into());
    }
    Ok(input_source)
}

fn select_input_source(input_source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = std::process::Command::new("macism")
        .arg(input_source)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("macism exited with {}", output.status).into())
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

fn handle_pane_focused(
    event_pane_id: Option<&str>,
    state: &mut State,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(focused_pane_id) = current_pane_id()? else {
        return Ok(());
    };
    if event_pane_id.is_some_and(|pane_id| pane_id != focused_pane_id) {
        return Ok(());
    }

    let observed_input_source = current_input_source()?;
    if current_pane_id()?.as_deref() != Some(&focused_pane_id) {
        return Ok(());
    }
    let mut next_state = state.clone();
    let target = crate::remember_focus(&mut next_state, &focused_pane_id, &observed_input_source);
    if current_pane_id()?.as_deref() != Some(&focused_pane_id) {
        return Ok(());
    }
    if let Some(target) = target {
        select_input_source(&target)?;
    }
    *state = next_state;
    Ok(())
}

fn update_state(
    command: &str,
    payload: &serde_json::Value,
    state: &mut State,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        "pane-focused" => handle_pane_focused(crate::pane_id_from(payload), state),
        "pane-closed" => {
            if let Some(pane_id) = crate::pane_id_from(payload) {
                state.panes.remove(pane_id);
                if state.last_focused_pane_id.as_deref() == Some(pane_id) {
                    state.last_focused_pane_id = None;
                }
            }
            Ok(())
        }
        _ => Err(format!("unknown command: {command}").into()),
    }
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
    fn reads_pane_id_from_event_envelope() {
        let payload = serde_json::json!({
            "event": "pane.focused",
            "data": {"pane_id": "w1:p2"}
        });
        assert_eq!(crate::pane_id_from(&payload), Some("w1:p2"));
    }

    #[test]
    fn first_focus_keeps_and_remembers_the_current_input_source() {
        let mut state = crate::State::default();

        let target = crate::remember_focus(&mut state, "w1:p1", "japanese");

        assert_eq!(target, None);
        assert_eq!(state.last_focused_pane_id.as_deref(), Some("w1:p1"));
        assert_eq!(state.panes["w1:p1"].input_source_id, "japanese");
    }

    #[test]
    fn new_pane_uses_abc_and_remembers_the_previous_pane() {
        let mut state = crate::State::default();
        crate::remember_focus(&mut state, "w1:p1", "japanese");

        let target = crate::remember_focus(&mut state, "w1:p2", "japanese");

        assert_eq!(target.as_deref(), Some(crate::DEFAULT_INPUT_SOURCE));
        assert_eq!(state.panes["w1:p1"].input_source_id, "japanese");
        assert_eq!(
            state.panes["w1:p2"].input_source_id,
            crate::DEFAULT_INPUT_SOURCE
        );
    }

    #[test]
    fn returning_to_a_pane_restores_its_input_source() {
        let mut state = crate::State::default();
        crate::remember_focus(&mut state, "w1:p1", "japanese");
        crate::remember_focus(&mut state, "w1:p2", "japanese");

        let target = crate::remember_focus(&mut state, "w1:p1", crate::DEFAULT_INPUT_SOURCE);

        assert_eq!(target.as_deref(), Some("japanese"));
        assert_eq!(
            state.panes["w1:p2"].input_source_id,
            crate::DEFAULT_INPUT_SOURCE
        );
    }

    #[test]
    fn repeated_focus_does_not_replace_existing_memory() {
        let mut state = crate::State::default();
        crate::remember_focus(&mut state, "w1:p1", "japanese");

        let target = crate::remember_focus(&mut state, "w1:p1", "abc");

        assert_eq!(target, None);
        assert_eq!(state.panes["w1:p1"].input_source_id, "japanese");
    }

    #[test]
    fn old_state_uses_last_pane_id_as_the_migration_baseline() {
        let state: crate::State = serde_json::from_value(serde_json::json!({
            "last_pane_id": "w1:p1",
            "last_agent": "codex",
            "new_pane_ids": []
        }))
        .expect("old state should deserialize");

        assert_eq!(state.last_focused_pane_id.as_deref(), Some("w1:p1"));
        assert!(state.panes.is_empty());
    }

    #[test]
    fn closing_a_pane_removes_its_memory() {
        let mut state = crate::State::default();
        crate::remember_focus(&mut state, "w1:p1", "japanese");
        let payload = serde_json::json!({
            "event": "pane.closed",
            "data": {"pane_id": "w1:p1"}
        });

        crate::update_state("pane-closed", &payload, &mut state)
            .expect("pane close should succeed");

        assert!(state.panes.is_empty());
        assert_eq!(state.last_focused_pane_id, None);
    }
}
