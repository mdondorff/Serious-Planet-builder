//! Tier A: the debug overlay is plan data (VIEW-014, CON-03): the lit plan carries it, a plan without one keeps the text and hash it had
//! before overlays existed, and the text form round-trips.

use planet_frame::look::{LitPlan, Overlay};

// spec: VIEW-014
#[test]
fn a_lit_plan_has_no_overlay_by_default_and_carries_the_one_it_is_given() {
    let plan = LitPlan::fixed(11.0);
    assert_eq!(plan.overlay, Overlay::None);
    assert_eq!(Overlay::default(), Overlay::None);
    let wire = plan.with_overlay(Overlay::Wireframe);
    assert_eq!(wire.overlay, Overlay::Wireframe);
    assert_eq!(plan.overlay, Overlay::None, "the plan it was made from is unchanged");
}

// spec: VIEW-014
#[test]
fn the_text_of_a_plan_without_an_overlay_is_what_it_was_before_and_a_wireframe_adds_one_token() {
    let plan = LitPlan::fixed(11.0);
    let base = plan.to_text();
    assert!(!base.contains("overlay"), "{base}");
    // The same plan with the overlay explicitly off writes the same text.
    assert_eq!(plan.with_overlay(Overlay::None).to_text(), base);
    let wire = plan.with_overlay(Overlay::Wireframe).to_text();
    assert_eq!(wire, format!("{base} overlay wireframe"));
}

// spec: VIEW-014
#[test]
fn the_overlay_round_trips_through_the_text_form_with_the_other_tokens_and_unknown_ones_are_errors() {
    let plans = [
        LitPlan::fixed(11.0).with_overlay(Overlay::Wireframe),
        LitPlan::adaptive(11.0, 0.016).with_water_level(12.5).with_overlay(Overlay::Wireframe),
        LitPlan::fixed(9.0),
    ];
    for plan in plans {
        let text = plan.to_text();
        let tokens: Vec<&str> = text.split_whitespace().collect();
        assert_eq!(LitPlan::from_tokens(&tokens[1..]).unwrap(), plan, "{text}");
        plan.validate().unwrap();
    }
    let text = LitPlan::fixed(11.0).to_text();
    let mut tokens: Vec<&str> = text.split_whitespace().collect();
    tokens.extend(["overlay", "sparkles"]);
    assert!(LitPlan::from_tokens(&tokens[1..]).unwrap_err().contains("sparkles"));
    let mut twice: Vec<&str> = text.split_whitespace().collect();
    twice.extend(["overlay", "wireframe", "overlay", "wireframe"]);
    assert!(LitPlan::from_tokens(&twice[1..]).is_err(), "at most once");
}

// spec: VIEW-014
#[test]
fn the_overlay_names_are_stable_and_parse_back() {
    for o in [Overlay::None, Overlay::Wireframe] {
        assert_eq!(Overlay::parse(o.name()), Some(o));
    }
    assert_eq!(Overlay::Wireframe.name(), "wireframe");
    assert_eq!(Overlay::parse("maps"), None);
}
