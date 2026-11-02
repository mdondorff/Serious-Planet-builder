//! REND-049, tier A: the march's resolution follows the sun. A ray per 4x4 pixels below 5 degrees, a ray per 2x2 from 5.5 degrees on, and between
//! them the resolution stays what it was, so a sun hovering at the switch does not remake the march targets every frame.

use planet_render::resolve::{
    march_scale_for_sun, spread_for_sun, HALF_RES_SCALE, QUARTER_ABOVE_SUN_DEG, QUARTER_ABOVE_SUN_SIN, QUARTER_BELOW_SUN_DEG,
    QUARTER_BELOW_SUN_SIN, QUARTER_RES_SCALE,
};

fn sin_deg(deg: f32) -> f32 {
    libm::sinf(deg.to_radians())
}

// spec: REND-049
#[test]
fn the_march_is_at_quarter_resolution_below_five_degrees_and_at_half_from_five_and_a_half() {
    for deg in [-10.0, 0.0, 0.2, 2.0, 4.9] {
        assert_eq!(march_scale_for_sun(HALF_RES_SCALE, sin_deg(deg)), QUARTER_RES_SCALE, "{deg} degrees");
        assert_eq!(march_scale_for_sun(QUARTER_RES_SCALE, sin_deg(deg)), QUARTER_RES_SCALE, "{deg} degrees");
    }
    for deg in [5.6, 10.0, 45.0, 90.0] {
        assert_eq!(march_scale_for_sun(HALF_RES_SCALE, sin_deg(deg)), HALF_RES_SCALE, "{deg} degrees");
        assert_eq!(march_scale_for_sun(QUARTER_RES_SCALE, sin_deg(deg)), HALF_RES_SCALE, "{deg} degrees");
    }
    // Between 5 and 5.5 degrees the resolution stays: a sun moving through the band switches once each way, never twice a frame.
    for deg in [5.0, 5.2, 5.4] {
        assert_eq!(march_scale_for_sun(HALF_RES_SCALE, sin_deg(deg)), HALF_RES_SCALE, "{deg} degrees, from half");
        assert_eq!(march_scale_for_sun(QUARTER_RES_SCALE, sin_deg(deg)), QUARTER_RES_SCALE, "{deg} degrees, from quarter");
    }
}

// spec: REND-049
#[test]
fn the_sines_are_the_angles_and_the_checkerboard_switches_at_the_lower_one() {
    assert!((QUARTER_BELOW_SUN_SIN - sin_deg(QUARTER_BELOW_SUN_DEG)).abs() < 1e-7);
    assert!((QUARTER_ABOVE_SUN_SIN - sin_deg(QUARTER_ABOVE_SUN_DEG)).abs() < 1e-7);
    // The spread (REND-050) and the quarter resolution begin at the same angle.
    assert!(spread_for_sun(sin_deg(4.99)) && !spread_for_sun(sin_deg(5.01)));
    assert_eq!(march_scale_for_sun(HALF_RES_SCALE, sin_deg(4.99)), QUARTER_RES_SCALE);
}
