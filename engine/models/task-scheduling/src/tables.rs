//! The production plan as tables for the page's data editor: every field the
//! model reads, so a person can change any of it without writing JSON.

use solverforge_workbench_host::data::{choice, col, list, Column, Table};

fn table(id: &'static str, label: &'static str, one: &'static str, columns: Vec<Column>) -> Table {
    Table {
        id,
        label,
        one,
        path: id,
        parent: "",
        title: "",
        columns,
    }
}

fn minutes(name: &'static str, label: &'static str) -> Column {
    col(name, label, "minutes").unit("minutes").optional()
}

fn named(id: &'static str, label: &'static str, one: &'static str) -> Table {
    Table {
        title: "name",
        ..table(id, label, one, vec![col("name", "Name", "text").optional()])
    }
}

pub fn tables() -> Vec<Table> {
    vec![
        Table {
            title: "name",
            ..table(
                "resources",
                "Machines and people",
                "machine or person",
                vec![
                    col("name", "Name", "text").optional(),
                    choice(
                        "kind",
                        "Kind",
                        &[("machine", "Machine"), ("employee", "Person")],
                    ),
                    col("machineType", "Machine type", "text")
                        .reference("machineTypes")
                        .optional(),
                    col("initialTaskType", "Set up for", "text")
                        .reference("taskTypes")
                        .optional(),
                    col("start", "Available from", "date_time"),
                    list(
                        "tags",
                        "Tags",
                        col("tag", "Tag", "object").fields(vec![
                            col("tag", "Tag", "text").reference("tags"),
                            col("from", "From", "date_time").optional(),
                            col("until", "Until", "date_time").optional(),
                        ]),
                    ),
                    list(
                        "unavailable",
                        "Unavailable",
                        col("span", "Span", "object").fields(vec![
                            col("start", "From", "date_time"),
                            col("end", "Until", "date_time"),
                            col("label", "Why", "text").optional(),
                        ]),
                    ),
                    col(
                        "freezeTasksUntil",
                        "Keep tasks starting before",
                        "date_time",
                    )
                    .optional(),
                    list(
                        "tasks",
                        "Runs",
                        col("run", "Run", "object").fields(vec![
                            col("task", "Task", "text").reference("tasks"),
                            col("start", "Starts", "date_time").optional(),
                            col("end", "Ends", "date_time").optional(),
                            col("changeoverStart", "Changeover from", "date_time").optional(),
                            minutes("changeoverDuration", "Changeover"),
                        ]),
                    )
                    .read_only(),
                    minutes("idleTime", "Idle").read_only(),
                    col("changeovers", "Changeovers", "integer").read_only(),
                    minutes("changeoverTime", "Changeover time").read_only(),
                ],
            )
        },
        Table {
            title: "name",
            ..table(
                "tasks",
                "Tasks",
                "task",
                vec![
                    col("name", "Name", "text").optional(),
                    minutes("duration", "Length"),
                    list(
                        "durationOn",
                        "Length on",
                        col("length", "Length", "object").fields(vec![
                            col("resource", "Machine or person", "text").reference("resources"),
                            col("duration", "Length", "minutes").unit("minutes"),
                        ]),
                    ),
                    col("taskType", "Task type", "text")
                        .reference("taskTypes")
                        .optional(),
                    col("minStart", "Not before", "date_time").optional(),
                    col("maxEnd", "Done by", "date_time").optional(),
                    col("idealEnd", "Ideally done at", "date_time").optional(),
                    col("dueDate", "Due", "date_time").optional(),
                    col("priority", "Priority", "integer")
                        .unit("points")
                        .optional(),
                    list(
                        "requiredResources",
                        "Only on",
                        col("resource", "Resource", "text").reference("resources"),
                    ),
                    list(
                        "requiredTags",
                        "Needs tags",
                        col("tag", "Tag", "text").reference("tags"),
                    ),
                    list(
                        "dependencies",
                        "After",
                        col("dependency", "Dependency", "object").fields(vec![
                            col("task", "Task", "text").reference("tasks"),
                            choice(
                                "anchor",
                                "Counted from its",
                                &[("end", "End"), ("start", "Start")],
                            )
                            .optional(),
                            minutes("minOffset", "At least"),
                            minutes("maxOffset", "At most"),
                            col("sameResource", "Same machine or person", "boolean").optional(),
                            col("noIntermediateTasks", "Nothing in between", "boolean").optional(),
                        ]),
                    ),
                    solverforge_workbench_host::stock::column("consumes", "Uses up"),
                    solverforge_workbench_host::stock::column("produces", "Makes"),
                    col("noInterruptions", "Runs without pausing", "boolean").optional(),
                    col("pinned", "Pinned", "boolean").optional(),
                ],
            )
        },
        table(
            "changeovers",
            "Changeovers",
            "changeover",
            vec![
                col("machineType", "Machine type", "text").reference("machineTypes"),
                col("from", "From task type", "text").reference("taskTypes"),
                col("to", "To task type", "text").reference("taskTypes"),
                col("duration", "Takes", "minutes").unit("minutes"),
            ],
        ),
        table(
            "resourceTransitions",
            "Hand-overs",
            "hand-over",
            vec![
                col("from", "From", "text").reference("resources"),
                col("to", "To", "text").reference("resources"),
                col("reward", "Reward", "integer").unit("points").optional(),
                col("forbidden", "Forbidden", "boolean").optional(),
            ],
        ),
        table(
            "stock",
            "Stock",
            "stock arrival",
            vec![
                col("part", "Part", "text"),
                col("quantity", "Quantity", "integer"),
                col("at", "Arrives", "date_time").optional(),
            ],
        ),
        named("tags", "Tags", "tag"),
        named("taskTypes", "Task types", "task type"),
        named("machineTypes", "Machine types", "machine type"),
    ]
}
