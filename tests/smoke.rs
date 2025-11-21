use mdminecraft_core::SimTick;
use mdminecraft_testkit::{EventRecord, JsonlSink};

#[test]
fn deterministic_event_stream_can_be_written() {
    let mut sink = JsonlSink::create(std::env::temp_dir().join("eventlog.jsonl"))
        .expect("can create temp log");
    let tick = SimTick::ZERO.advance(1);
    let record = EventRecord {
        tick,
        kind: "SmokeTest",
        payload: "ok",
    };
    sink.write(&record).expect("can write event");
}
