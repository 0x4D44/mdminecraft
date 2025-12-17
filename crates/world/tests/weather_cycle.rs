//! Weather cycle worldtest scenario.
//!
//! Validates deterministic weather transitions and time-of-day progression.

use mdminecraft_world::{SimTime, WeatherChanged, WeatherState, WeatherToggle};

#[test]
fn weather_cycle_is_deterministic() {
    let mut time = SimTime::default();
    let mut weather = WeatherToggle::new();

    // Initial state verification.
    assert_eq!(time.time_of_day(), 0.0, "Should start at midnight");
    assert_eq!(weather.state, WeatherState::Clear);

    // Advance through one game day (24000 ticks).
    for _ in 0..24000 {
        time.advance();
    }

    // Verify time wraps back to midnight.
    assert!(
        (time.time_of_day() - 0.0).abs() < 0.001,
        "Time should wrap at day boundary"
    );

    // Toggle weather to precipitation.
    let old_state = weather.state;
    weather.toggle();
    let event = WeatherChanged::new(old_state, weather.state);

    assert_eq!(event.from, WeatherState::Clear);
    assert_eq!(event.to, WeatherState::Precipitation);
    assert_eq!(weather.state, WeatherState::Precipitation);
    assert!(weather.is_precipitating());

    // Advance to noon (half day = 12000 ticks).
    for _ in 0..12000 {
        time.advance();
    }

    // Verify noon time.
    assert!(
        (time.time_of_day() - 0.5).abs() < 0.01,
        "Should be noon after 12000 additional ticks"
    );

    // Toggle weather back to clear.
    let old_state = weather.state;
    weather.toggle();
    let event = WeatherChanged::new(old_state, weather.state);

    assert_eq!(event.from, WeatherState::Precipitation);
    assert_eq!(event.to, WeatherState::Clear);
    assert_eq!(weather.state, WeatherState::Clear);
    assert!(!weather.is_precipitating());
}

#[test]
fn skylight_varies_with_time_and_weather() {
    let mut time = SimTime::default();
    let mut weather = WeatherToggle::new();

    // Midnight, clear: minimum skylight.
    let midnight_clear = time.effective_skylight();
    assert!(
        (3..=4).contains(&midnight_clear),
        "Midnight should have ~20% light, got {}",
        midnight_clear
    );

    // Advance to dawn (6000 ticks = 1/4 day) where light difference is more visible.
    for _ in 0..6000 {
        time.advance();
    }

    // Dawn, clear.
    weather.set_state(WeatherState::Clear);
    let dawn_clear = time.effective_skylight();

    // Dawn, precipitation: reduced.
    weather.set_state(WeatherState::Precipitation);
    let dawn_modified = time.skylight_scalar() * weather.skylight_modifier();
    let dawn_rain = (15.0 * dawn_modified).round() as u8;
    assert!(
        dawn_rain < dawn_clear,
        "Rain should reduce light at dawn: {} vs {}",
        dawn_rain,
        dawn_clear
    );

    // Noon, clear: maximum skylight (advance 6000 more ticks from dawn to noon).
    for _ in 0..6000 {
        time.advance();
    }
    weather.set_state(WeatherState::Clear);
    let noon_clear = time.effective_skylight();
    assert_eq!(noon_clear, 15, "Noon should have full light");

    // Noon, precipitation: slightly reduced.
    weather.set_state(WeatherState::Precipitation);
    let noon_modified = time.skylight_scalar() * weather.skylight_modifier();
    let noon_rain = (15.0 * noon_modified).round() as u8;
    assert!(
        (12..15).contains(&noon_rain),
        "Noon rain should have ~85% light, got {}",
        noon_rain
    );
}

#[test]
fn time_advancement_is_deterministic() {
    let mut time1 = SimTime::new(24000);
    let mut time2 = SimTime::new(24000);

    // Both start at same tick.
    assert_eq!(time1.tick, time2.tick);

    // Advance both by same amount.
    for _ in 0..1000 {
        time1.advance();
        time2.advance();
    }

    // Both should be identical.
    assert_eq!(time1.tick, time2.tick);
    assert_eq!(time1.time_of_day(), time2.time_of_day());
    assert_eq!(time1.skylight_scalar(), time2.skylight_scalar());
}
