//! The pick-up and delivery plan as tables for the page's data editor: every
//! field the model reads, so a person can change any of it without writing
//! JSON.

use solverforge_workbench_host::data::{choice, col, list, windows, Column, Table};
use solverforge_workbench_host::priority_options;

fn skills(name: &'static str, label: &'static str, level: &'static str) -> Column {
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
    list(name, label, col("skill", "Skill", "object").fields(fields))
}

fn time(name: &'static str, label: &'static str) -> Column {
    col(name, label, "date_time").optional()
}

fn texts(name: &'static str, label: &'static str, one: &'static str) -> Column {
    list(name, label, col("item", one, "text"))
}

fn drivers(name: &'static str, label: &'static str) -> Column {
    list(
        name,
        label,
        col("driver", "Driver", "text").reference("drivers"),
    )
}

fn seconds(name: &'static str, label: &'static str) -> Column {
    col(name, label, "integer").unit("s").optional()
}

/// A lunch break or an appointment: fixed at a time, or floating inside a
/// window for a number of minutes.
fn breaks() -> Column {
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
            col("duration", "Length", "minutes")
                .unit("minutes")
                .optional()
                .when("type", &["FLOATING"]),
            col("location", "Place", "location").optional(),
            choice(
                "policy",
                "Jobs on board",
                &[
                    ("PERMIT_JOBS", "May stay on board"),
                    ("REJECT_JOBS", "None on board"),
                ],
            )
            .optional(),
        ]),
    )
}

/// The solved route: stops and breaks in driving order, with their times.
fn itinerary() -> Column {
    list(
        "itinerary",
        "Route",
        col("item", "Stop", "object").fields(vec![
            col("id", "Stop", "text"),
            col("kind", "Kind", "text"),
            time("arrivalTime", "Arrives"),
            time("startServiceTime", "Starts"),
            time("departureTime", "Leaves"),
            seconds("travelSeconds", "Driving"),
            col("travelMeters", "Distance", "integer")
                .unit("m")
                .optional(),
            col("pin", "Pinned", "boolean").optional(),
            col("unreachable", "No road", "boolean").optional(),
            list(
                "load",
                "Load",
                col("load", "Load", "object").fields(vec![
                    col("type", "Kind", "text"),
                    col("previous", "Before", "integer"),
                    col("current", "After", "integer"),
                ]),
            ),
        ]),
    )
    .read_only()
}

fn metrics() -> Column {
    col("metrics", "Totals", "object")
        .fields(vec![
            time("startTime", "Sets off"),
            time("endLocationArrivalTime", "Back"),
            time("lastStopDepartureTime", "Leaves the last stop"),
            seconds("travelSeconds", "Driving"),
            col("travelMeters", "Distance", "integer")
                .unit("m")
                .optional(),
            seconds("overtimeSeconds", "Overtime"),
        ])
        .read_only()
}

pub fn tables() -> Vec<Table> {
    vec![
        Table {
            id: "drivers",
            label: "Drivers",
            one: "driver",
            path: "drivers",
            parent: "",
            title: "name",
            columns: vec![col("name", "Name", "text").optional()],
        },
        Table {
            id: "shifts",
            label: "Shifts",
            one: "shift",
            path: "drivers/*/shifts",
            parent: "drivers",
            title: "",
            columns: vec![
                time("minStartTime", "Starts"),
                time("maxEndTime", "Ends by"),
                time("maxSoftEndTime", "Should end by"),
                time("minFirstStopArrivalTime", "First stop not before"),
                time("maxLastStopDepartureTime", "Leaves the last stop by"),
                time(
                    "maxSoftLastStopDepartureTime",
                    "Should leave the last stop by",
                ),
                col("startLocation", "Starts at", "location"),
                col("endLocation", "Ends at", "location").optional(),
                breaks(),
                list(
                    "capacities",
                    "Capacity",
                    col("capacity", "Capacity", "object").fields(vec![
                        col("type", "Kind", "text"),
                        col("quantity", "How many", "integer"),
                    ]),
                ),
                skills("skills", "Skills", "level"),
                texts("tags", "Tags", "Tag"),
                col("pinned", "Keep the route as it is", "boolean").optional(),
                col(
                    "pinNextStopDuringFreeze",
                    "Keep the next stop when replanning",
                    "boolean",
                )
                .optional(),
                itinerary(),
                metrics(),
                col("unreachable", "A leg has no road", "boolean").read_only(),
            ],
        },
        Table {
            id: "jobs",
            label: "Jobs",
            one: "job",
            path: "jobs",
            parent: "",
            title: "name",
            columns: vec![
                col("name", "Name", "text").optional(),
                col("priority", "Priority", "text")
                    .options(priority_options())
                    .optional(),
                skills("requiredSkills", "Needs skills", "minLevel"),
                texts("requiredTags", "Needs tags", "Tag"),
                drivers("requiredDrivers", "Only drivers"),
                drivers("preferredDrivers", "Preferred drivers"),
                drivers("prohibitedDrivers", "Never drivers"),
                list(
                    "demands",
                    "Takes up",
                    col("demand", "Demand", "object").fields(vec![
                        col("type", "Kind", "text"),
                        col("demand", "How many", "integer"),
                    ]),
                ),
                col("prohibitJobPooling", "Rides alone", "boolean").optional(),
                list(
                    "prohibitedJobs",
                    "Never on board with",
                    col("job", "Job", "text").reference("jobs"),
                ),
                col("maximumTimeBurden", "Most extra ride time", "minutes")
                    .unit("minutes")
                    .optional(),
                choice(
                    "allowInterruption",
                    "Breaks while on board",
                    &[("BREAKS", "Allowed"), ("NONE", "Not allowed")],
                )
                .optional(),
            ],
        },
        Table {
            id: "stops",
            label: "Stops",
            one: "stop",
            path: "jobs/*/stops",
            parent: "jobs",
            title: "name",
            columns: vec![
                col("name", "Name", "text").optional(),
                col("location", "Place", "location"),
                col("duration", "Minutes there", "minutes").unit("minutes"),
                windows(
                    "timeWindows",
                    "Time windows",
                    &[
                        ("minStartTime", "From"),
                        ("maxStartTime", "Start by"),
                        ("maxEndTime", "Done by"),
                    ],
                ),
                time("latestSlaEndTime", "Should end by"),
                time("minStartTravelTime", "Set off towards it no earlier than"),
                list(
                    "demands",
                    "Loads and unloads",
                    col("demand", "Demand", "object").fields(vec![
                        col("type", "Kind", "text"),
                        col("pickup", "Picks up", "integer").optional(),
                        col("deliver", "Delivers", "integer").optional(),
                    ]),
                ),
            ],
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
