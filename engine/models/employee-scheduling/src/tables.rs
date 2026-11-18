//! The roster as tables for the page's data editor: every field the model
//! reads, so a person can change any of it without writing JSON.

use solverforge_workbench_host::data::{choice, col, list, Column, Table};

const LEVELS: &[(&str, &str)] = &[("required", "Required"), ("preferred", "Preferred")];
const STANCES: &[(&str, &str)] = &[
    ("required", "Required"),
    ("preferred", "Preferred"),
    ("unwanted", "Unwanted"),
    ("barred", "Barred"),
];
const WEEKDAYS: &[(&str, &str)] = &[
    ("monday", "Monday"),
    ("tuesday", "Tuesday"),
    ("wednesday", "Wednesday"),
    ("thursday", "Thursday"),
    ("friday", "Friday"),
    ("saturday", "Saturday"),
    ("sunday", "Sunday"),
];
const RULE_KINDS: &[(&str, &str)] = &[
    ("minutesWorked", "Minutes worked"),
    ("minutesLogged", "Minutes logged"),
    ("daysWorked", "Days worked"),
    ("shiftsWorked", "Shifts worked"),
    ("weekendsWorked", "Weekends worked"),
    ("weekendMinutes", "Weekend minutes"),
    ("consecutiveWeekendsWorked", "Weekends worked in a row"),
    ("consecutiveDaysWorked", "Days worked in a row"),
    ("consecutiveShifts", "Shifts in a row"),
    ("daysOff", "Days off"),
    ("consecutiveDaysOff", "Days off in a row"),
    ("consecutiveMinutesOff", "Minutes off in a row"),
    ("consecutiveWeekendsOff", "Weekends off in a row"),
    ("nearDayOff", "Shifts next to a day off"),
    ("overlapAllowed", "Overlapping shifts"),
    ("restBetween", "Rest between shifts"),
    ("startSpread", "Spread of start times"),
    ("splitShifts", "Split shifts"),
    ("dailyPairing", "Shift pairs days apart"),
    ("dayPattern", "Pattern within a day"),
    ("multiDayPattern", "Pattern over days"),
    ("rotation", "Rotation"),
    ("shiftTypes", "Shift types"),
    ("distinctTags", "Distinct tags"),
    ("travel", "Travel"),
    ("locationsWorked", "Locations worked"),
    ("cost", "Cost"),
    ("disruption", "Changes to the published roster"),
];

fn texts(name: &'static str, label: &'static str, one: &'static str) -> Column {
    list(name, label, col("item", one, "text"))
}

fn employees(name: &'static str, label: &'static str) -> Column {
    list(
        name,
        label,
        col("employee", "Employee", "text").reference("employees"),
    )
}

fn weekdays(name: &'static str, label: &'static str) -> Column {
    list(name, label, choice("day", "Day", WEEKDAYS))
}

/// A tag filter's fields, as a record carries them inline.
fn filter_fields() -> Vec<Column> {
    vec![
        texts("withTags", "With tags", "Tag"),
        texts("withoutTags", "Without tags", "Tag"),
        col("matchAll", "Every tag", "boolean").optional(),
    ]
}

/// A tag filter as a field of its own.
fn filter(name: &'static str, label: &'static str) -> Column {
    col(name, label, "object")
        .fields(filter_fields())
        .optional()
}

fn with_filter(mut fields: Vec<Column>) -> Vec<Column> {
    fields.extend(filter_fields());
    fields
}

fn place(name: &'static str, label: &'static str) -> Column {
    col(name, label, "object")
        .fields(vec![
            col("lat", "Latitude", "number").unit("°"),
            col("lon", "Longitude", "number").unit("°"),
        ])
        .optional()
}

fn span(name: &'static str, label: &'static str) -> Column {
    let mut fields = with_filter(vec![
        col("from", "From", "date_time"),
        col("to", "Until", "date_time"),
    ]);
    fields.push(col("weight", "Weight", "integer").optional());
    list(name, label, col("span", "Span", "object").fields(fields))
}

fn level() -> Column {
    choice("level", "Level", LEVELS).optional()
}

fn weight() -> Column {
    col("weight", "Weight", "integer").optional()
}

fn range() -> [Column; 2] {
    [
        col("min", "At least", "integer").optional(),
        col("max", "At most", "integer").optional(),
    ]
}

fn minutes(name: &'static str, label: &'static str) -> Column {
    col(name, label, "minutes").unit("minutes").optional()
}

fn flag(name: &'static str, label: &'static str) -> Column {
    col(name, label, "boolean").optional()
}

fn filters(name: &'static str, label: &'static str, one: &'static str) -> Column {
    list(
        name,
        label,
        col("item", one, "object").fields(filter_fields()),
    )
}

/// One limit of a contract: its kind decides which of the other fields it reads.
fn rule() -> Vec<Column> {
    let mut rule = with_filter(vec![choice("kind", "Rule", RULE_KINDS), level(), weight()]);
    rule.extend([
        col("period", "Period", "text").optional(),
        col("rollingDays", "Rolling days", "integer")
            .unit("days")
            .optional(),
    ]);
    rule.extend(range());
    rule.extend([
        flag("wholeShift", "Whole shift inside the window"),
        weekdays("weekdays", "Weekdays"),
        texts("categories", "Categories", "Category"),
        weekdays("startDays", "Runs start on"),
        minutes("minGapMinutes", "Gap between categories"),
        flag("compact", "Keep categories together"),
        minutes("linkMinutes", "Shifts this close are one run"),
        minutes("restAfterMinutes", "Rest after a run"),
        col("restAfterRunsOf", "Rest after runs of", "integer")
            .unit("shifts")
            .optional(),
        choice("stance", "Stance", STANCES).optional(),
        minutes("withinMinutes", "Pairs within"),
        filter("paired", "Paired shift"),
        col("dayOffset", "Days later", "integer")
            .unit("days")
            .optional(),
        filters("steps", "Steps", "Step"),
        flag("ordered", "Steps in order"),
        list(
            "days",
            "Days",
            col("day", "Day", "object").fields(with_filter(vec![flag("work", "Works")])),
        ),
        filters("groups", "Groups", "Group"),
        choice(
            "cycle",
            "Cycle",
            &[("weekly", "Weekly"), ("blocks", "Blocks of days")],
        )
        .optional(),
        texts("tagTypes", "Tag types", "Tag type"),
        col("maxMeters", "Farthest from home", "integer")
            .unit("m")
            .optional(),
        minutes("minChangeMinutes", "Time to change place"),
        col("hourly", "Hourly rate", "number").optional(),
        minutes("baseMinutes", "Minutes at the base rate"),
        list(
            "overtime",
            "Overtime tiers",
            col("tier", "Tier", "object").fields(vec![
                col("hourly", "Hourly rate", "number"),
                minutes("minutes", "For"),
            ]),
        ),
        texts("keepTags", "Tags to keep", "Tag"),
        filter("before", "Shift before"),
        filter("after", "Shift after"),
    ]);
    rule
}

/// A schedule-wide rule: its own fields, the shifts it counts and the
/// employees whose shifts count.
fn schedule_rule(fields: Vec<Column>) -> Vec<Column> {
    let mut columns = with_filter(fields);
    columns.push(filter("employees", "Employees"));
    columns
}

fn table(
    id: &'static str,
    label: &'static str,
    one: &'static str,
    path: &'static str,
    columns: Vec<Column>,
) -> Table {
    Table {
        id,
        label,
        one,
        path,
        parent: "",
        title: "",
        columns,
    }
}

pub fn tables() -> Vec<Table> {
    vec![
        Table {
            title: "name",
            ..table(
                "employees",
                "Employees",
                "employee",
                "employees",
                vec![
                    col("name", "Name", "text").optional(),
                    col("skills", "Skills", "json").optional(),
                    list(
                        "contracts",
                        "Contracts",
                        col("contract", "Contract", "text").reference("contracts"),
                    ),
                    texts("tags", "Tags", "Tag"),
                    col("priority", "Priority", "integer").optional(),
                    texts("avoidRisks", "Must avoid", "Risk"),
                    span("unavailable", "Unavailable"),
                    span("availableOnly", "Available only"),
                    span("preferredTimes", "Would like to work"),
                    span("unwantedTimes", "Would rather not work"),
                    list("daysOff", "Days off", col("day", "Day", "date")),
                    filter("requiredShifts", "May only work"),
                    filter("preferredShifts", "Likes"),
                    list(
                        "pairings",
                        "Works with",
                        col("pairing", "Pairing", "object").fields(vec![
                            choice("stance", "Stance", STANCES),
                            col("with", "Employee", "text")
                                .reference("employees")
                                .optional(),
                            texts("withTags", "Or anyone tagged", "Tag"),
                            texts("onTags", "On shifts tagged", "Tag"),
                        ]),
                    ),
                    place("home", "Home"),
                    col("costGroup", "Cost group", "text")
                        .reference("costGroups")
                        .optional(),
                    list(
                        "loggedBefore",
                        "Logged before",
                        col("log", "Log", "object").fields(vec![
                            col("date", "Date", "date"),
                            col("minutes", "Minutes", "minutes").unit("minutes"),
                        ]),
                    ),
                ],
            )
        },
        table(
            "shifts",
            "Shifts",
            "shift",
            "shifts",
            vec![
                col("start", "Starts", "date_time"),
                col("end", "Ends", "date_time"),
                col("employee", "Employee", "text")
                    .reference("employees")
                    .optional(),
                flag("pinned", "Pinned"),
                list(
                    "breaks",
                    "Breaks",
                    col("break", "Break", "object").fields(vec![
                        col("from", "From", "date_time"),
                        col("to", "Until", "date_time"),
                    ]),
                ),
                col("requiredSkills", "Needs skills", "json").optional(),
                flag("anyRequiredSkill", "Any one skill is enough"),
                col("preferredSkills", "Would like skills", "json").optional(),
                texts("tags", "Tags", "Tag"),
                texts("risks", "Risks", "Risk"),
                flag("optional", "Optional"),
                col("importance", "Importance", "integer").optional(),
                employees("preferredEmployees", "Would like"),
                employees("unwantedEmployees", "Would rather not have"),
                employees("barredEmployees", "Never"),
                place("location", "Place"),
                col("costGroup", "Cost group", "text").optional(),
                col("alternativeOf", "Version of", "text")
                    .reference("alternatives")
                    .optional(),
                col("published", "Published to", "text")
                    .reference("employees")
                    .optional(),
            ],
        ),
        table("contracts", "Contracts", "contract", "contracts", vec![]),
        Table {
            parent: "contracts",
            title: "kind",
            ..table(
                "rules",
                "Contract rules",
                "rule",
                "contracts/*/rules",
                rule(),
            )
        },
        table(
            "schedule",
            "Calendar",
            "calendar",
            "schedule",
            vec![
                col("start", "Plans from", "date_time"),
                col("end", "Plans until", "date_time"),
                choice("weekStart", "Weeks start on", WEEKDAYS).optional(),
                weekdays("weekend", "Weekend"),
                list(
                    "periods",
                    "Periods",
                    col("period", "Period", "object").fields(vec![
                        col("id", "Name", "text"),
                        list(
                            "ranges",
                            "Ranges",
                            col("range", "Range", "object").fields(vec![
                                col("from", "From", "date"),
                                col("to", "Until", "date"),
                            ]),
                        ),
                    ]),
                ),
            ],
        ),
        table(
            "demand",
            "Demand",
            "demand rule",
            "scheduleRules/demand",
            schedule_rule(vec![
                level(),
                list(
                    "slots",
                    "Slots",
                    col("slot", "Slot", "object").fields({
                        let mut slot = vec![
                            col("from", "From", "date_time"),
                            col("to", "Until", "date_time"),
                        ];
                        slot.extend(range());
                        slot
                    }),
                ),
            ]),
        ),
        table(
            "shiftCounts",
            "Shift counts",
            "count rule",
            "scheduleRules/shiftCounts",
            schedule_rule({
                let mut c = vec![col("period", "Period", "text"), level(), weight()];
                c.extend(range());
                c
            }),
        ),
        table(
            "distinctTags",
            "Distinct tags",
            "distinct-tags rule",
            "scheduleRules/distinctTags",
            {
                let mut c = with_filter(vec![
                    col("period", "Period", "text"),
                    level(),
                    weight(),
                    texts("tagTypes", "Tag types", "Tag type"),
                ]);
                c.extend(range());
                c
            },
        ),
        table(
            "concurrency",
            "At the same time",
            "concurrency rule",
            "scheduleRules/concurrency",
            {
                let mut c = with_filter(vec![col("max", "At most", "integer")]);
                c.push(col("from", "From", "date_time").optional());
                c.push(col("to", "Until", "date_time").optional());
                c
            },
        ),
        table(
            "costs",
            "Cost limits",
            "cost rule",
            "scheduleRules/costs",
            schedule_rule({
                let mut c = vec![
                    col("period", "Period", "text"),
                    level(),
                    weight(),
                    list(
                        "rates",
                        "Hourly rates",
                        col("rate", "Rate", "object").fields(vec![
                            col("employeeGroup", "Employee group", "text").optional(),
                            col("shiftGroup", "Shift group", "text").optional(),
                            col("hourly", "Hourly", "number"),
                        ]),
                    ),
                ];
                c.extend(range());
                c
            }),
        ),
        table(
            "fairness",
            "Fairness",
            "fairness rule",
            "scheduleRules/fairness",
            schedule_rule(vec![
                choice(
                    "balance",
                    "Balance",
                    &[("shifts", "Shifts"), ("minutes", "Minutes")],
                ),
                col("history", "Already worked, by employee", "json").optional(),
            ]),
        ),
        table(
            "alternatives",
            "Alternative shifts",
            "alternatives group",
            "alternatives",
            vec![
                col("count", "Versions to work", "integer"),
                flag("atMost", "At most"),
            ],
        ),
        table(
            "costGroups",
            "Cost groups",
            "cost group",
            "costGroups",
            vec![
                col("activationCost", "Cost to bring in", "integer").optional(),
                col("share", "Share", "integer").optional(),
                flag("fillUp", "Fill up who is in"),
            ],
        ),
        table(
            "teams",
            "Teams",
            "team",
            "teams",
            vec![
                employees("leads", "Leads"),
                employees("members", "Members"),
                col("tagType", "Members share a", "text").optional(),
                col("maxShiftsPerDay", "Shift times a day", "integer").optional(),
            ],
        ),
        table(
            "tags",
            "Tag types",
            "tag",
            "tags",
            vec![col("type", "Type", "text")],
        ),
    ]
}
