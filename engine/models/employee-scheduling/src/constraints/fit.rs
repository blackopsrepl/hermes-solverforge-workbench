//! Skills, risk factors and availability: facts of one shift and its employee.

use solverforge::prelude::*;

use super::assignment::{bonus, hard, soft, Score};
use crate::derived::Derived;
use crate::domain::Plan;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        hard("Required skill lacking", Derived::required_skills_missing),
        soft("Preferred skill lacking", Derived::preferred_skills_missing),
        hard("Prohibited risk exposure", Derived::risks_hit),
        hard("Works while unavailable", Derived::unavailable_minutes),
        hard(
            "Works outside available hours",
            Derived::outside_available_minutes,
        ),
        bonus("Works in preferred hours", Derived::preferred_minutes),
        soft("Works in unwanted hours", Derived::unwanted_minutes),
        bonus("Preferred employee on shift", Derived::preferred_employee),
        soft("Unwanted employee on shift", Derived::unwanted_employee),
        hard("Barred employee on shift", Derived::barred_employee),
        hard(
            "Shift outside employee's required tags",
            Derived::outside_required_shifts,
        ),
        bonus(
            "Shift has employee's preferred tags",
            Derived::inside_preferred_shifts,
        ),
    )
}
