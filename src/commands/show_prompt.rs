use crate::metrics::db::MetricsDatabase;

/// Handle the `show-prompt` command
///
/// Usage: `git-ai show-prompt <trace_id>`
///
/// Returns the session events (prompts/transcripts) from the local metrics database
/// for the session associated with the given trace ID.
pub fn handle_show_prompt(args: &[String]) {
    let parsed = match parse_args(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let db = match MetricsDatabase::global() {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open metrics database: {}", e);
            std::process::exit(1);
        }
    };

    let db_guard = db.lock().unwrap();

    // 1. Resolve trace_id or session_id to session_id
    let session_id = if parsed.prompt_id.starts_with("s_") {
        parsed.prompt_id.split("::").next().unwrap().to_string()
    } else {
        match db_guard.get_session_id_by_trace_id(&parsed.prompt_id) {
            Ok(Some(sid)) => sid,
            Ok(None) => {
                eprintln!("Error: Trace ID not found or has no associated session in local metrics DB: {}", parsed.prompt_id);
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Error querying metrics database: {}", e);
                std::process::exit(1);
            }
        }
    };

    // 2. Fetch all SessionEvents for that session
    match db_guard.get_session_events(&session_id) {
        Ok(events) => {
            if events.is_empty() {
                eprintln!("No prompt events found for session {}", session_id);
                std::process::exit(1);
            }

            let mut parsed_events = Vec::new();
            for event_str in events {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&event_str) {
                    parsed_events.push(json);
                }
            }

            let output = serde_json::json!({
                "trace_id": parsed.prompt_id,
                "session_id": session_id,
                "session_events": parsed_events,
            });

            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            );
        }
        Err(e) => {
            eprintln!("Error querying session events: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Debug)]
pub struct ParsedArgs {
    pub prompt_id: String,
    pub commit: Option<String>,
    pub offset: usize,
}

pub fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let mut prompt_id: Option<String> = None;
    let mut commit: Option<String> = None;
    let mut offset: Option<usize> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--commit" {
            if i + 1 >= args.len() {
                return Err("--commit requires a value".to_string());
            }
            i += 1;
            commit = Some(args[i].clone());
        } else if arg == "--offset" {
            if i + 1 >= args.len() {
                return Err("--offset requires a value".to_string());
            }
            i += 1;
            offset = Some(
                args[i]
                    .parse::<usize>()
                    .map_err(|_| "--offset must be a non-negative integer")?,
            );
        } else if arg.starts_with('-') {
            return Err(format!("Unknown option: {}", arg));
        } else {
            if prompt_id.is_some() {
                return Err("Only one prompt ID can be specified".to_string());
            }
            prompt_id = Some(arg.clone());
        }

        i += 1;
    }

    let prompt_id = prompt_id.ok_or("show-prompt requires a prompt ID")?;

    // Validate mutual exclusivity of --commit and --offset
    if commit.is_some() && offset.is_some() {
        return Err("--commit and --offset are mutually exclusive".to_string());
    }

    Ok(ParsedArgs {
        prompt_id,
        commit,
        offset: offset.unwrap_or(0),
    })
}
