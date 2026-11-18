//! The roster as tables for the page's data editor.

use solverforge_workbench_host::data::{col, list, Table};

pub fn tables() -> Vec<Table> {
    let span = |name, label| {
        list(
            name,
            label,
            col("span", "Span", "object").fields(vec![
                col("from", "From", "date_time"),
                col("to", "Until", "date_time"),
            ]),
        )
    };
    vec![
        Table {
            id: "employees",
            label: "Employees",
            one: "employee",
            path: "employees",
            parent: "",
            title: "name",
            columns: vec![
                col("name", "Name", "text").optional(),
                col("skills", "Skills", "json").optional(),
                list(
                    "contracts",
                    "Contracts",
                    col("contract", "Contract", "text"),
                ),
                list("tags", "Tags", col("tag", "Tag", "text")),
                span("unavailable", "Unavailable"),
                span("preferredTimes", "Would like to work"),
                span("unwantedTimes", "Would rather not work"),
            ],
        },
        Table {
            id: "shifts",
            label: "Shifts",
            one: "shift",
            path: "shifts",
            parent: "",
            title: "",
            columns: vec![
                col("start", "Starts", "date_time"),
                col("end", "Ends", "date_time"),
                col("employee", "Employee", "text")
                    .reference("employees")
                    .optional(),
                col("pinned", "Pinned", "boolean").optional(),
                col("requiredSkills", "Needs skills", "json").optional(),
                list("tags", "Tags", col("tag", "Tag", "text")),
            ],
        },
    ]
}
