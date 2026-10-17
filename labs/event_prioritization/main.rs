use std::collections::HashSet;

#[derive(Debug)]
struct ProcessSnapshot {
    pid: u32,
    name: String,
}

#[derive(Debug, PartialEq)]
enum ProcessEvent {
    Started { pid: u32, name: String },
    Exited { pid: u32, name: String },
}

fn detect_lifecycle_events(
    previous: &[ProcessSnapshot],
    current: &[ProcessSnapshot],
) -> Vec<ProcessEvent> {
    let previous_pids: HashSet<u32> =
        previous.iter().map(|process| process.pid).collect();

    let current_pids: HashSet<u32> =
        current.iter().map(|process| process.pid).collect();

    let mut events = Vec::new();

    for process in current {
        if !previous_pids.contains(&process.pid) {
            events.push(ProcessEvent::Started {
                pid: process.pid,
                name: process.name.clone(),
            });
        }
    }

    for process in previous {
        if !current_pids.contains(&process.pid) {
            events.push(ProcessEvent::Exited {
                pid: process.pid,
                name: process.name.clone(),
            });
        }
    }

    events
}

fn main() {
    let previous = vec![
        ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        },
        ProcessSnapshot {
            pid: 200,
            name: "sleep".into(),
        },
    ];

    let current = vec![
        ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        },
        ProcessSnapshot {
            pid: 300,
            name: "python".into(),
        },
    ];

    let events = detect_lifecycle_events(&previous, &current);

    for event in events {
        println!("{event:?}");
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_started_process() {
        let previous = vec![];
        let current = vec![ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        }];

        let events = detect_lifecycle_events(&previous, &current);

        assert_eq!(
            events,
            vec![ProcessEvent::Started {
                pid: 100,
                name: "bash".into(),
            }]
        );
    }

    #[test]
    fn detects_exited_process() {
        let previous = vec![ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        }];
        let current = vec![];

        let events = detect_lifecycle_events(&previous, &current);

        assert_eq!(
            events,
            vec![ProcessEvent::Exited {
                pid: 100,
                name: "bash".into(),
            }]
        );
    }

    #[test]
    fn unchanged_process_produces_no_event() {
        let previous = vec![ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        }];
        let current = vec![ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        }];

        let events = detect_lifecycle_events(&previous, &current);

        assert!(events.is_empty());
    }

    #[test]
    fn detects_start_and_exit_together() {
        let previous = vec![ProcessSnapshot {
            pid: 100,
            name: "bash".into(),
        }];
        let current = vec![ProcessSnapshot {
            pid: 200,
            name: "python".into(),
        }];

        let events = detect_lifecycle_events(&previous, &current);

        assert_eq!(events.len(), 2);
        assert!(events.contains(&ProcessEvent::Started {
            pid: 200,
            name: "python".into(),
        }));
        assert!(events.contains(&ProcessEvent::Exited {
            pid: 100,
            name: "bash".into(),
        }));
    }
}