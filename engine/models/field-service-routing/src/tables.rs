//! The field service plan as tables for the page's data editor.

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
                col("historicalTimeUtilized", "Worked before", "minutes")
                    .unit("minutes")
                    .optional(),
                col("historicalTimeCapacity", "Available before", "minutes")
                    .unit("minutes")
                    .optional(),
            ],
        },
        Table {
            id: "shifts",
            label: "Shifts",
            one: "shift",
            path: "vehicles/*/shifts",
            parent: "technicians",
            title: "",
            columns: vec![
                col("minStartTime", "Starts", "date_time").optional(),
                col("maxEndTime", "Ends by", "date_time").optional(),
                col("startLocation", "Starts at", "location"),
                col("endLocation", "Ends at", "location").optional(),
                skills("skills", "Skills", "level"),
                list("tags", "Tags", col("tag", "Tag", "text")),
            ],
        },
        Table {
            id: "visits",
            label: "Visits",
            one: "visit",
            path: "visits",
            parent: "",
            title: "name",
            columns: vec![
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
                skills("requiredSkills", "Needs skills", "minLevel"),
                list("requiredTags", "Needs tags", col("tag", "Tag", "text")),
                list(
                    "requiredVehicles",
                    "Only technicians",
                    col("technician", "Technician", "text").reference("technicians"),
                ),
            ],
        },
    ]
}
