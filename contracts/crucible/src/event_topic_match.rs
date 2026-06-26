// Shared helpers for matching Soroban event topics.
//
// NOTE: This module exists to ensure all public event-filtering helpers use the
// same topic comparison strategy (payload equality, not debug-string matching).

use soroban_sdk::{Env, Val, Vec as SorobanVec};

pub(crate) fn topics_match_by_payload(
    filter_topics: &SorobanVec<Val>,
    event_topics: &SorobanVec<Val>,
) -> bool {
    if event_topics.len() < filter_topics.len() {
        return false;
    }

    filter_topics.iter().enumerate().all(|(i, filter_topic)| {
        // Val doesn't implement PartialEq; compare raw bit payloads.
        let ev_topic = event_topics.get(i as u32).unwrap();
        filter_topic.get_payload() == ev_topic.get_payload()
    })
}
