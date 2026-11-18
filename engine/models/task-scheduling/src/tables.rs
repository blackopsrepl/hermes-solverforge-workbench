//! The production plan as tables for the page's data editor.

use solverforge_workbench_host::data::{col, list, Table};
use solverforge_workbench_host::ParameterOption;

pub fn tables() -> Vec<Table> {
    vec![
        Table {
            id: "resources",
            label: "Machines and people",
            one: "machine or person",
            path: "resources",
            parent: "",
            title: "name",
            columns: vec![
                col("name", "Name", "text").optional(),
                col("kind", "Kind", "text").options(vec![
                    ParameterOption::new("machine", "Machine"),
                    ParameterOption::new("employee", "Person"),
                ]),
                col("machineType", "Machine type", "text").optional(),
                col("start", "Available from", "date_time"),
                list(
                    "unavailable",
                    "Unavailable",
                    col("span", "Span", "object").fields(vec![
                        col("start", "From", "date_time"),
                        col("end", "Until", "date_time"),
                        col("label", "Why", "text").optional(),
                    ]),
                ),
            ],
        },
        Table {
            id: "tasks",
            label: "Tasks",
            one: "task",
            path: "tasks",
            parent: "",
            title: "name",
            columns: vec![
                col("name", "Name", "text").optional(),
                col("duration", "Length", "minutes")
                    .unit("minutes")
                    .optional(),
                col("minStart", "Not before", "date_time").optional(),
                col("maxEnd", "Done by", "date_time").optional(),
                col("dueDate", "Due", "date_time").optional(),
                col("priority", "Priority", "integer")
                    .unit("points")
                    .optional(),
                list(
                    "requiredResources",
                    "Only on",
                    col("resource", "Resource", "text").reference("resources"),
                ),
                list("requiredTags", "Needs tags", col("tag", "Tag", "text")),
                col("dependencies", "Dependencies", "json").optional(),
            ],
        },
    ]
}
