//! The pick-up and delivery plan as tables for the page's data editor.

use solverforge_workbench_host::data::{col, list, windows, Column, Table};
use solverforge_workbench_host::priority_options;

fn skills(name: &'static str, label: &'static str, level: &'static str) -> Column {
    list(
        name,
        label,
        col("skill", "Skill", "object").fields(vec![
            col("name", "Skill", "text"),
            col(level, "Level", "integer").optional(),
        ]),
    )
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
                col("minStartTime", "Starts", "date_time").optional(),
                col("maxEndTime", "Ends by", "date_time").optional(),
                col("startLocation", "Starts at", "location"),
                col("endLocation", "Ends at", "location").optional(),
                list(
                    "capacities",
                    "Capacity",
                    col("capacity", "Capacity", "object").fields(vec![
                        col("type", "Kind", "text"),
                        col("quantity", "How many", "integer"),
                    ]),
                ),
                skills("skills", "Skills", "level"),
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
                list("requiredTags", "Needs tags", col("tag", "Tag", "text")),
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
            ],
        },
    ]
}
