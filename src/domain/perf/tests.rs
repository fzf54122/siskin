use super::*;

fn sample(frame_ms: u64) -> PerfSample {
    PerfSample {
        frame_time: Duration::from_millis(frame_ms),
        shape_time: Duration::from_millis(1),
        frame_build_time: Duration::from_millis(2),
        pty_events: 10,
        sessions_refreshed: 1,
        draw_tasks: 100,
        shape_hits: 90,
        shape_misses: 10,
    }
}

#[test]
fn disabled_recorder_records_nothing() {
    let mut recorder = PerfRecorder::new(false);
    recorder.record(sample(16));
    assert_eq!(recorder.summary().frames, 0);
    recorder.set_enabled(true);
    recorder.record(sample(16));
    assert_eq!(recorder.summary().frames, 1);
    recorder.set_enabled(false);
    assert_eq!(recorder.summary().frames, 0);
}

#[test]
fn samples_are_capped_so_long_output_cannot_grow_forever() {
    let mut recorder = PerfRecorder::new(true);
    for _ in 0..(MAX_SAMPLES + 100) {
        recorder.record(sample(16));
    }
    assert_eq!(recorder.summary().frames, MAX_SAMPLES);
}

#[test]
fn percentile_uses_sorted_frame_times() {
    let mut recorder = PerfRecorder::new(true);
    for ms in [10, 10, 10, 10, 10, 10, 10, 10, 10, 100] {
        recorder.record(sample(ms));
    }
    let summary = recorder.summary();
    assert_eq!(summary.frame_p50_ms, 10.);
    assert_eq!(summary.frame_p95_ms, 100.);
    assert_eq!(summary.frame_max_ms, 100.);
}

#[test]
fn summary_averages_shape_build_events_and_draws() {
    let mut recorder = PerfRecorder::new(true);
    recorder.record(sample(16));
    recorder.record(sample(16));
    let summary = recorder.summary();
    assert_eq!(summary.frames, 2);
    assert_eq!(summary.shape_avg_ms, 1.);
    assert_eq!(summary.frame_build_avg_ms, 2.);
    assert_eq!(summary.pty_events_per_frame, 10.);
    assert_eq!(summary.sessions_per_frame, 1.);
    assert_eq!(summary.draw_tasks_per_frame, 100.);
    assert!((summary.shape_hit_rate - 0.9).abs() < 1e-9);
}

#[test]
fn note_render_fills_the_latest_sample_instead_of_adding_one() {
    let mut recorder = PerfRecorder::new(true);
    recorder.record(sample(16));
    recorder.note_render(Duration::from_millis(3), 42, 90, 10);
    assert_eq!(recorder.summary().frames, 1);
    let summary = recorder.summary();
    assert_eq!(summary.shape_avg_ms, 3.);
    assert_eq!(summary.draw_tasks_per_frame, 42.);
}

#[test]
fn fps_is_derived_from_sample_count_and_total_time() {
    let mut recorder = PerfRecorder::new(true);
    for _ in 0..10 {
        recorder.record(sample(100));
    }
    let summary = recorder.summary();
    assert!((summary.fps - 10.).abs() < 1e-6);
}

#[test]
fn env_flag_only_accepts_explicit_truthy_values() {
    assert!(parse_env_flag(&Some("1".to_string())));
    assert!(parse_env_flag(&Some("TRUE".to_string())));
    assert!(parse_env_flag(&Some("on".to_string())));
    assert!(!parse_env_flag(&None));
    assert!(!parse_env_flag(&Some("0".to_string())));
    assert!(!parse_env_flag(&Some("false".to_string())));
    assert!(!parse_env_flag(&Some("".to_string())));
}

#[test]
fn empty_summary_is_zero_not_nan() {
    let recorder = PerfRecorder::new(true);
    let summary = recorder.summary();
    assert_eq!(summary.frames, 0);
    assert!(summary.fps.is_finite());
    assert!(summary.frame_p95_ms.is_finite());
    assert!(summary.one_line().contains("FPS"));
}
