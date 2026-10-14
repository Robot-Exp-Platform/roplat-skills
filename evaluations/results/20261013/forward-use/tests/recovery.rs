use roplat_forward_test::{
    mock_arm::{Event, Journal},
    nodes::AttemptStatus,
    position_and_retire,
};

#[tokio::test(flavor = "current_thread")]
async fn right_failure_preserves_left_motion_and_allows_both_recovery_motions() {
    let journal = Journal::default();
    let report = position_and_retire(([0.5; 2], [2.0; 2]), journal.clone())
        .await
        .unwrap();
    assert!(
        matches!(report.attempt, Some(AttemptStatus::Failed(ref message)) if message.contains("right target exceeds mock joint limits"))
    );
    assert_eq!((report.left, report.right), ([-0.25; 2], [0.25; 2]));
    assert_eq!(report.connected_in_tail, (true, true));
    let events = journal.events();
    assert_eq!(events.len(), 8);
    assert_eq!(
        &events[..4],
        &[
            Event::Connected("left"),
            Event::Connected("right"),
            Event::Accepted("left", [0.5; 2]),
            Event::Rejected("right", [2.0; 2]),
        ]
    );
    assert!(events[4..6].contains(&Event::Accepted("left", [-0.25; 2])));
    assert!(events[4..6].contains(&Event::Accepted("right", [0.25; 2])));
    assert_eq!(
        &events[6..],
        &[Event::Disconnected("right"), Event::Disconnected("left")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn left_failure_skips_second_fixture_motion_but_reuses_both_resources() {
    let journal = Journal::default();
    let report = position_and_retire(([2.0; 2], [0.5; 2]), journal.clone())
        .await
        .unwrap();
    assert!(
        matches!(report.attempt, Some(AttemptStatus::Failed(ref message)) if message.contains("left target exceeds mock joint limits"))
    );
    assert_eq!((report.left, report.right), ([-0.25; 2], [0.25; 2]));
    let events = journal.events();
    assert_eq!(events.len(), 7);
    assert!(!events.contains(&Event::Accepted("right", [0.5; 2])));
    assert!(events[3..5].contains(&Event::Accepted("left", [-0.25; 2])));
    assert!(events[3..5].contains(&Event::Accepted("right", [0.25; 2])));
    assert_eq!(
        &events[5..],
        &[Event::Disconnected("right"), Event::Disconnected("left")]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn successful_fixture_uses_distinct_same_typed_arms_then_retires_normally() {
    let journal = Journal::default();
    let report = position_and_retire(([0.5; 2], [-0.5; 2]), journal.clone())
        .await
        .unwrap();
    assert_eq!(report.attempt, Some(AttemptStatus::Completed));
    assert_eq!((report.left, report.right), ([-0.25; 2], [0.25; 2]));
    assert_eq!(report.connected_in_tail, (true, true));
    let events = journal.events();
    assert_eq!(events.len(), 8);
    assert_eq!(
        &events[..4],
        &[
            Event::Connected("left"),
            Event::Connected("right"),
            Event::Accepted("left", [0.5; 2]),
            Event::Accepted("right", [-0.5; 2]),
        ]
    );
    assert_eq!(
        &events[6..],
        &[Event::Disconnected("right"), Event::Disconnected("left")]
    );
}
