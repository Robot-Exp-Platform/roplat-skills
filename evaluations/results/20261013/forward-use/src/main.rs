use roplat_forward_test::{
    mock_arm::{Event, Journal},
    nodes::AttemptStatus,
    position_and_retire,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let journal = Journal::default();
    let report = position_and_retire(([0.5; 2], [2.0; 2]), journal.clone())
        .await
        .expect("parent handles fixture failure and parks both resources");
    assert!(
        matches!(report.attempt, Some(AttemptStatus::Failed(ref message)) if message.contains("right target exceeds mock joint limits"))
    );
    assert_eq!(report.left, [-0.25; 2]);
    assert_eq!(report.right, [0.25; 2]);
    assert_eq!(report.connected_in_tail, (true, true));
    let events = journal.events();
    assert_eq!(
        &events[..4],
        &[
            Event::Connected("left"),
            Event::Connected("right"),
            Event::Accepted("left", [0.5; 2]),
            Event::Rejected("right", [2.0; 2]),
        ]
    );
    assert_eq!(
        &events[events.len() - 2..],
        &[Event::Disconnected("right"), Event::Disconnected("left"),]
    );
    println!(
        "Handled fixture error, parked both resources, and closed in reverse order.\n{report:#?}\n{events:#?}"
    );
}
