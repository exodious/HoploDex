//! specs/005-regulated-item-types research.md §7: the built-in form names,
//! used only as suggestions for `registration_form`; nothing here is stored.

/// data-model.md "Built-in Form Name", in rank order: a bare "F" or "Form"
/// suggests Form 4 first. Only the forms a personal collection files (spec
/// clarification: Forms 2, 3 and 10 are for licensed dealers and manufacturers,
/// so they are not offered; an owner may still type one).
pub const BUILT_IN_FORMS: &[&str] = &["Form 4", "Form 1", "Form 5"];
