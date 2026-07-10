//! Event sink + console command tests.

use ashfall_bridge::hooks;

// Event type constants (matching NVSE event dispatcher)
const EVENT_ON_HIT: u32 = 1;
const EVENT_ON_DEATH: u32 = 5;

// Global counter for callback verification
// ponytail: raw static for extern "C" callback — callbacks MUST NOT panic
static mut CALLBACK_HIT_FIRED: u32 = 0;
static mut CALLBACK_DEATH_FIRED: u32 = 0;
static mut LAST_EVENT_TYPE: u32 = 0;
static mut LAST_ARG0: u32 = 0;
static mut LAST_ARG1: u32 = 0;

extern "C" fn test_callback_hit(event_type: u32, arg0: u32, arg1: u32, _arg2: u32) {
    unsafe {
        CALLBACK_HIT_FIRED += 1;
        LAST_EVENT_TYPE = event_type;
        LAST_ARG0 = arg0;
        LAST_ARG1 = arg1;
    }
}

extern "C" fn test_callback_death(_event_type: u32, _arg0: u32, _arg1: u32, _arg2: u32) {
    unsafe { CALLBACK_DEATH_FIRED += 1; }
}

fn reset_counters() {
    unsafe {
        CALLBACK_HIT_FIRED = 0;
        CALLBACK_DEATH_FIRED = 0;
        LAST_EVENT_TYPE = 0;
        LAST_ARG0 = 0;
        LAST_ARG1 = 0;
    }
}

#[test]
fn test_event_sink_registration() {
    // Cleanup from other tests
    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_hit);
    // May have stale empty entry — has_event_sinks now checks non-empty
    assert!(!hooks::has_event_sinks(EVENT_ON_HIT));

    hooks::register_event_sink(EVENT_ON_HIT, test_callback_hit);
    assert!(hooks::has_event_sinks(EVENT_ON_HIT));

    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_hit);
    assert!(!hooks::has_event_sinks(EVENT_ON_HIT));
}

#[test]
fn test_event_sink_dispatch() {
    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_hit);
    hooks::unregister_event_sink(EVENT_ON_DEATH, test_callback_death);
    reset_counters();

    hooks::register_event_sink(EVENT_ON_HIT, test_callback_hit);
    hooks::register_event_sink(EVENT_ON_HIT, test_callback_death);

    let count = hooks::dispatch_event(EVENT_ON_HIT, 0x42, 0x99, 0);
    assert_eq!(count, 2);
    unsafe {
        assert_eq!(CALLBACK_HIT_FIRED, 1);
        assert_eq!(CALLBACK_DEATH_FIRED, 1);
        assert_eq!(LAST_EVENT_TYPE, EVENT_ON_HIT);
        assert_eq!(LAST_ARG0, 0x42);
        assert_eq!(LAST_ARG1, 0x99);
    }

    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_hit);
    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_death);
}

#[test]
fn test_event_sink_unknown_type() {
    assert!(!hooks::has_event_sinks(99));
    let count = hooks::dispatch_event(99, 0, 0, 0);
    assert_eq!(count, 0);
}

#[test]
fn test_event_sink_multiple_types() {
    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_hit);
    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_death);
    hooks::unregister_event_sink(EVENT_ON_DEATH, test_callback_death);

    hooks::register_event_sink(EVENT_ON_HIT, test_callback_hit);
    hooks::register_event_sink(EVENT_ON_DEATH, test_callback_death);

    assert!(hooks::has_event_sinks(EVENT_ON_HIT));
    assert!(hooks::has_event_sinks(EVENT_ON_DEATH));

    reset_counters();
    let count = hooks::dispatch_event(EVENT_ON_HIT, 0, 0, 0);
    assert_eq!(count, 1);
    unsafe { assert_eq!(CALLBACK_HIT_FIRED, 1); }

    hooks::unregister_event_sink(EVENT_ON_HIT, test_callback_hit);
    hooks::unregister_event_sink(EVENT_ON_DEATH, test_callback_death);
}

#[test]
fn test_console_command_registration() {
    // Cleanup from other tests
    hooks::unregister_console_command("/kick");
    hooks::unregister_console_command("/ban");
    hooks::unregister_console_command("/msg");

    assert!(!hooks::has_console_command("/kick"));

    hooks::register_console_command("/kick");
    assert!(hooks::has_console_command("/kick"));
    assert!(hooks::hook_console_command("/kick"));

    assert!(!hooks::has_console_command("/ban"));

    hooks::unregister_console_command("/kick");
    assert!(!hooks::has_console_command("/kick"));
    assert!(!hooks::hook_console_command("/kick"));
}

#[test]
fn test_console_command_dispatch() {
    hooks::register_console_command("/kick");
    hooks::register_console_command("/ban");
    hooks::register_console_command("/msg");

    assert!(hooks::hook_console_command("/kick"));
    assert!(hooks::hook_console_command("/ban"));
    assert!(hooks::hook_console_command("/msg"));

    assert!(!hooks::hook_console_command("/players"));
    assert!(!hooks::hook_console_command("/help"));

    hooks::unregister_console_command("/kick");
    hooks::unregister_console_command("/ban");
    hooks::unregister_console_command("/msg");
}

#[test]
fn test_console_command_case_sensitive() {
    hooks::unregister_console_command("/Kick");
    hooks::unregister_console_command("/kick");

    hooks::register_console_command("/Kick");

    assert!(hooks::hook_console_command("/Kick"));
    assert!(!hooks::hook_console_command("/kick"));

    hooks::unregister_console_command("/Kick");
}
