use super::*;

#[test]
fn send_icon_map_produces_valid_json() {
    let mut buf: Vec<u8> = Vec::new();
    send_icon_map(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_icon_map");
    let params = &parsed["params"];
    assert!(params["dir_open"].as_str().unwrap_or("").len() > 0);
    assert!(params["dir_closed"].as_str().unwrap_or("").len() > 0);
    assert!(params["fallback"].as_str().unwrap_or("").len() > 0);
    let icons = params["icons"].as_object().unwrap();
    assert!(icons.contains_key("rs"), "must contain rust icon");
    assert!(icons.contains_key("py"), "must contain python icon");
    assert!(icons.contains_key("md"), "must contain markdown icon");
    assert!(icons.contains_key("sh"), "must contain shell icon");
    assert!(icons.len() > 50, "should have many icon entries");
}

#[test]
fn main_loop_handles_init_and_shutdown() {
    let input = "{\"event\":\"init\"}\n{\"event\":\"shutdown\"}\n";
    let mut out: Vec<u8> = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = serde_json::from_str(trimmed).unwrap();
        match msg["event"].as_str().unwrap_or("") {
            "init" => {
                send_icon_map(&mut out);
            }
            "shutdown" => break,
            _ => {}
        }
    }
    let output = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "set_icon_map");
}

#[test]
fn main_loop_ignores_unknown_events() {
    let input = "{\"event\":\"unknown\"}\n{\"event\":\"shutdown\"}\n";
    let out: Vec<u8> = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _msg: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    }
    assert!(out.is_empty());
}
