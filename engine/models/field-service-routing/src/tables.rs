//! The field service plan as tables for the page's data editor: every field
//! the model reads, so a person can change any of it without writing JSON.

use solverforge_workbench_host::data::{choice, col, list, windows, Column, Table};
use solverforge_workbench_host::priority_options;

fn skills(name: &'static str, label: &'static str, level: &'static str) -> Column {
    list(
        name,
        label,
        col("skill", "Skill", "object").fields(skill_fields(level)),
    )
}

fn skill_fields(level: &'static str) -> Vec<Column> {
    let mut fields = vec![
        col("name", "Skill", "text"),
        col(level, "Level", "integer").optional(),
    ];
    if level == "level" {
        fields.push(
            col("multiplier", "Service time", "number")
                .unit("×")
                .optional(),
        );
    }
    fields
}

fn time(name: &'static str, label: &'static str) -> Column {
    col(name, label, "date_time").optional()
}

fn minutes(name: &'static str, label: &'static str) -> Column {
    col(name, label, "minutes").unit("minutes").optional()
}

fn count(name: &'static str, label: &'static str) -> Column {
    col(name, label, "integer").unit("visits").optional()
}

fn texts(name: &'static str, label: &'static str, one: &'static str) -> Column {
    list(name, label, col("item", one, "text"))
}

fn technicians(name: &'static str, label: &'static str) -> Column {
    list(
        name,
        label,
        col("technician", "Technician", "text").reference("technicians"),
    )
}

fn seconds(name: &'static str, label: &'static str) -> Column {
    col(name, label, "integer").unit("s").optional()
}

/// Polygons, each a ring of corners.
fn area(name: &'static str, label: &'static str) -> Column {
    list(
        name,
        label,
        list("polygon", "Polygon", col("corner", "Corner", "location")),
    )
}

/// What a shift, or a run of linked shifts, offers and how long it may work.
fn limits() -> Vec<Column> {
    vec![
        list(
            "requiredBreaks",
            "Breaks",
            col("break", "Break", "object").fields(vec![
                col("id", "Name", "text"),
                choice(
                    "type",
                    "Kind",
                    &[("FIXED", "At a set time"), ("FLOATING", "Within a window")],
                ),
                time("startTime", "Starts").when("type", &["FIXED"]),
                time("endTime", "Ends").when("type", &["FIXED"]),
                time("minStartTime", "Not before").when("type", &["FLOATING"]),
                time("maxEndTime", "Done by").when("type", &["FLOATING"]),
                minutes("duration", "Length").when("type", &["FLOATING"]),
                col("location", "Place", "location").optional(),
            ]),
        ),
        skills("skills", "Skills", "level"),
        texts("tags", "Tags", "Tag"),
        list(
            "temporarySkillSets",
            "Skills for part of the shift",
            col("set", "Skills", "object").fields(vec![
                col("start", "From", "date_time"),
                col("end", "Until", "date_time"),
                skills("skills", "Skills", "level"),
            ]),
        ),
        list(
            "temporaryTagSets",
            "Tags for part of the shift",
            col("set", "Tags", "object").fields(vec![
                col("start", "From", "date_time"),
                col("end", "Until", "date_time"),
                texts("tags", "Tags", "Tag"),
            ]),
        ),
        minutes("maxTravelTimePerVisit", "Longest drive to a visit"),
        minutes("maxTravelTime", "Most driving"),
        minutes("maxSoftTravelTime", "Should drive at most"),
        minutes("maxServiceDuration", "Most work"),
        minutes("maxSoftServiceDuration", "Should work at most"),
        count("maxVisits", "Most visits"),
        count("maxSoftVisits", "Should make at most"),
        col(
            "movableOccupationRatioThreshold",
            "Share movable visits may fill",
            "number",
        )
        .optional(),
        col("cost", "Cost", "object")
            .fields(vec![
                col("fixedCost", "Once it works", "number").optional(),
                list(
                    "rates",
                    "Rates",
                    col("rate", "Rate", "object").fields(vec![
                        minutes("duration", "For"),
                        col("activationCost", "On starting", "number").optional(),
                        col("costPerUnit", "Per unit", "number"),
                        choice("unit", "Unit", &[("MINUTE", "Minute"), ("HOUR", "Hour")]),
                    ]),
                ),
            ])
            .optional(),
        col("travelTimeAdjustment", "Drive times", "object")
            .fields(vec![
                col("multiplier", "Scaled by", "number")
                    .unit("×")
                    .optional(),
                minutes("extraTime", "Plus"),
            ])
            .optional(),
        col(
            "preferGroupingColocatedVisitsRadiusMeters",
            "One place within",
            "integer",
        )
        .unit("m")
        .optional(),
        col(
            "pinNextVisitDuringFreeze",
            "Keep the next visit when replanning",
            "boolean",
        )
        .optional(),
        col("pinned", "Keep the route as it is", "boolean").optional(),
        itinerary(),
        col("metrics", "Totals", "object")
            .fields(vec![
                time("startTime", "Sets off"),
                time("endLocationArrivalTime", "Back"),
                time("lastVisitDepartureTime", "Leaves the last visit"),
                seconds("travelSeconds", "Driving"),
                col("travelMeters", "Distance", "integer")
                    .unit("m")
                    .optional(),
                seconds("serviceSeconds", "Working"),
                seconds("waitingSeconds", "Waiting"),
                seconds("breakSeconds", "On break"),
                seconds("overtimeSeconds", "Overtime"),
                col("cost", "Cost", "number").optional(),
            ])
            .read_only(),
        col("unreachable", "A leg has no road", "boolean").read_only(),
    ]
}

/// The solved route: visits and breaks in driving order, with their times.
fn itinerary() -> Column {
    list(
        "itinerary",
        "Route",
        col("item", "Visit", "object").fields(vec![
            col("id", "Visit", "text"),
            col("kind", "Kind", "text"),
            time("arrivalTime", "Arrives"),
            time("startServiceTime", "Starts"),
            time("departureTime", "Leaves"),
            seconds("travelSeconds", "Driving"),
            col("travelMeters", "Distance", "integer")
                .unit("m")
                .optional(),
            seconds("effectiveServiceSeconds", "Working"),
            col("pin", "Pinned", "boolean").optional(),
            col("unreachable", "No road", "boolean").optional(),
        ]),
    )
    .read_only()
}

fn visit() -> Vec<Column> {
    vec![
        col("name", "Name", "text").optional(),
        col("location", "Place", "location"),
        col("serviceDuration", "Length", "minutes").unit("minutes"),
        col("priority", "Priority", "text")
            .options(priority_options())
            .optional(),
        windows(
            "timeWindows",
            "Time windows",
            &[
                ("minStartTime", "From"),
                ("maxStartTime", "Start by"),
                ("maxEndTime", "Done by"),
            ],
        ),
        windows(
            "preferredTimeWindows",
            "Preferred time windows",
            &[
                ("minStartTime", "From"),
                ("maxStartTime", "Start by"),
                ("maxEndTime", "Done by"),
            ],
        ),
        time("latestSlaEndTime", "Should end by"),
        time("minStartTravelTime", "Set off towards it no earlier than"),
        skills("requiredSkills", "Needs skills", "minLevel"),
        texts("requiredTags", "Needs tags", "Tag"),
        technicians("requiredVehicles", "Only technicians"),
        technicians("preferredVehicles", "Preferred technicians"),
        list(
            "preferredVehiclesWeights",
            "Preferred technicians, weighted",
            col("preference", "Preference", "object").fields(vec![
                col("id", "Technician", "text").reference("technicians"),
                col("weight", "Weight", "integer").optional(),
            ]),
        ),
        technicians("prohibitedVehicles", "Never technicians"),
        list(
            "serviceDurationBreakdown",
            "Length by skill",
            col("part", "Part", "object").fields(vec![
                col("skill", "Skill", "text"),
                col("duration", "Minutes", "minutes").unit("minutes"),
            ]),
        ),
        minutes("serviceDurationAdjustment", "Length adjusted by"),
        minutes(
            "durationAddedForFirstVisitOnLocation",
            "Arrival time at a new place",
        ),
        list(
            "visitDependencies",
            "After",
            col("dependency", "Dependency", "object").fields(vec![
                col("id", "Name", "text"),
                col("precedingVisit", "Visit", "text").reference("visits"),
                minutes("minDelay", "At least"),
                minutes("maxDelay", "At most"),
                delay("minDelayTo", "No earlier than"),
                delay("maxDelayTo", "No later than"),
                choice(
                    "coordination",
                    "Same",
                    &[
                        ("NONE", "Any technician"),
                        ("SAME_VEHICLE", "Same technician"),
                        ("SAME_SHIFT", "Same shift"),
                    ],
                )
                .optional(),
                choice(
                    "poolingType",
                    "Other visits in between",
                    &[
                        ("ALLOW_POOLING_ALL", "Any"),
                        ("ALLOW_POOLING_VISITS", "Only visits with no dependency"),
                        ("ALLOW_POOLING_DEPENDENCIES", "Only other dependencies"),
                        ("DISALLOW_POOLING", "None"),
                    ],
                )
                .optional(),
            ]),
        ),
        col("preferredConsecutiveGroupId", "Run with", "text").optional(),
        list(
            "exclusions",
            "Never planned with",
            col("visit", "Visit", "text").reference("visits"),
        ),
        col("profit", "Earns", "number").optional(),
    ]
}

/// A time of a later day.
fn delay(name: &'static str, label: &'static str) -> Column {
    col(name, label, "object")
        .fields(vec![
            choice(
                "adjuster",
                "Day",
                &[
                    ("NEXT_DAY", "Next day"),
                    ("NEXT_WORKING_DAY", "Next working day"),
                ],
            ),
            col("increment", "Days on", "integer")
                .unit("days")
                .optional(),
            col("time", "At", "text"),
        ])
        .optional()
}

pub fn tables() -> Vec<Table> {
    let mut shift = vec![
        time("minStartTime", "Starts"),
        time("maxEndTime", "Ends by"),
        time("maxSoftEndTime", "Should end by"),
        time("minFirstVisitArrivalTime", "First visit not before"),
        time("maxLastVisitDepartureTime", "Leaves the last visit by"),
        time(
            "maxSoftLastVisitDepartureTime",
            "Should leave the last visit by",
        ),
        col("startLocation", "Starts at", "location"),
        col("endLocation", "Ends at", "location").optional(),
        area("requiredArea", "Works only in"),
        area("preferredArea", "Prefers to work in"),
    ];
    shift.extend(limits());
    let mut linked = vec![list(
        "shifts",
        "Links",
        col("link", "Link", "object").fields(vec![
            col("id", "Name", "text"),
            col("fromShiftId", "From shift", "text").reference("shifts"),
            col("toShiftId", "To shift", "text").reference("shifts"),
        ]),
    )];
    linked.extend(limits());
    vec![
        Table {
            id: "technicians",
            label: "Technicians",
            one: "technician",
            path: "vehicles",
            parent: "",
            title: "name",
            columns: vec![
                col("name", "Name", "text").optional(),
                col("technicianRating", "Rating", "number")
                    .unit("points")
                    .optional(),
                minutes("historicalTimeUtilized", "Worked before"),
                minutes("historicalTimeCapacity", "Available before"),
                area("requiredArea", "Works only in"),
                area("preferredArea", "Prefers to work in"),
            ],
        },
        Table {
            id: "shifts",
            label: "Shifts",
            one: "shift",
            path: "vehicles/*/shifts",
            parent: "technicians",
            title: "",
            columns: shift,
        },
        Table {
            id: "trips",
            label: "Overnight trips",
            one: "trip",
            path: "vehicles/*/linkedShifts",
            parent: "technicians",
            title: "",
            columns: linked,
        },
        Table {
            id: "visits",
            label: "Visits",
            one: "visit",
            path: "visits",
            parent: "",
            title: "name",
            columns: visit(),
        },
        Table {
            id: "visitGroups",
            label: "Joint visits",
            one: "joint visit",
            path: "visitGroups",
            parent: "",
            title: "",
            columns: vec![choice(
                "alignment",
                "Together at",
                &[("START", "The start"), ("END", "The end")],
            )
            .optional()],
        },
        Table {
            id: "groupVisits",
            label: "Joint visit parts",
            one: "joint visit part",
            path: "visitGroups/*/visits",
            parent: "visitGroups",
            title: "name",
            columns: visit(),
        },
        Table {
            id: "planningWindow",
            label: "Planning window",
            one: "planning window",
            path: "planningWindow",
            parent: "",
            title: "",
            columns: vec![
                col("startDate", "From", "date_time"),
                col("endDate", "Until", "date_time"),
            ],
        },
        solverforge_workbench_host::traffic::table(),
    ]
}
