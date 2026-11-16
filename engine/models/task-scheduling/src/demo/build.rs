//! A small builder for demo datasets.

use std::sync::Arc;

use crate::derived::Facts;
use crate::domain::{Lane, Plan, Resource, Task};
use crate::time::{format_date_time, parse_date_time, DAY};
use crate::wire::{
    Anchor, Assignment, Changeover, Dependency, DurationOn, Parameters, ResourceKind,
    ResourceTransition, Span, TagDef, TagRef, TypeDef,
};

/// Monday 1 March 2027, 00:00: day 0 of every demo.
const MONDAY: &str = "2027-03-01T00:00";

/// Day `day` (0 = Monday) at `hh:mm`, as a local date-time.
pub fn at(day: i64, hour: i64, minute: i64) -> String {
    let base = parse_date_time(MONDAY).expect("a valid base date");
    format_date_time(base + day * DAY + hour * 60 + minute)
}

/// A dependency on `task` that starts after its end.
pub fn after(task: &str) -> Dependency {
    Dependency {
        task: task.to_string(),
        anchor: Anchor::End,
        min_offset: None,
        max_offset: None,
        same_resource: false,
        no_intermediate_tasks: false,
    }
}

/// A task with a default duration, nothing else set.
pub fn task(id: &str, name: &str, minutes: i64) -> Task {
    Task {
        index: 0,
        id: id.to_string(),
        name: Some(name.to_string()),
        duration: Some(minutes),
        duration_on: Vec::new(),
        required_tags: Vec::new(),
        required_resources: Vec::new(),
        dependencies: Vec::new(),
        min_start: None,
        max_end: None,
        ideal_end: None,
        due_date: None,
        priority: None,
        task_type: None,
        no_interruptions: None,
        pinned: false,
        pinned_start: None,
        frozen_on: None,
        frozen_previous: None,
        frozen_rank: 0,
        frozen_last: false,
        open_cost: 0,
        link_cost: 0,
        lane: None,
        previous: None,
        next: None,
        slot: None,
    }
}

/// Collects the records of one dataset.
pub struct Builder {
    plan: Plan,
}

impl Builder {
    pub fn new(name: &str) -> Self {
        Self {
            plan: Plan {
                name: Some(name.to_string()),
                tags: Vec::new(),
                task_types: Vec::new(),
                machine_types: Vec::new(),
                changeovers: Vec::new(),
                resource_transitions: Vec::new(),
                parameters: Parameters::default(),
                tasks: Vec::new(),
                resources: Vec::new(),
                facts: Arc::new(Facts::default()),
                weights: Default::default(),
                score: None,
            },
        }
    }

    pub fn tag(&mut self, id: &str) {
        self.plan.tags.push(TagDef {
            id: id.to_string(),
            name: None,
        });
    }

    pub fn task_type(&mut self, id: &str) {
        self.plan.task_types.push(TypeDef {
            id: id.to_string(),
            name: None,
        });
    }

    pub fn machine_type(&mut self, id: &str) {
        self.plan.machine_types.push(TypeDef {
            id: id.to_string(),
            name: None,
        });
    }

    pub fn changeover(&mut self, machine_type: &str, from: &str, to: &str, minutes: i64) {
        self.plan.changeovers.push(Changeover {
            machine_type: machine_type.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            duration: minutes,
        });
    }

    pub fn hand_over(&mut self, from: &str, to: &str, reward: Option<i64>) {
        self.plan.resource_transitions.push(ResourceTransition {
            from: from.to_string(),
            to: to.to_string(),
            reward,
            forbidden: reward.is_none(),
        });
    }

    fn resource(
        &mut self,
        id: &str,
        name: &str,
        kind: ResourceKind,
        start: String,
    ) -> &mut Resource {
        self.plan.resources.push(Resource {
            index: 0,
            id: id.to_string(),
            name: Some(name.to_string()),
            kind,
            start,
            machine_type: None,
            initial_task_type: None,
            tags: Vec::new(),
            unavailable: Vec::new(),
            freeze_tasks_until: None,
            tasks: Vec::new(),
            frozen: Vec::new(),
            sequence: Vec::new(),
            lane: Lane::Resource,
            pinned: false,
        });
        self.plan.resources.last_mut().expect("just pushed")
    }

    /// A machine of `machine_type`, available from `start`.
    pub fn machine(
        &mut self,
        id: &str,
        name: &str,
        machine_type: &str,
        start: String,
    ) -> &mut Resource {
        let resource = self.resource(id, name, ResourceKind::Machine, start);
        resource.machine_type = Some(machine_type.to_string());
        resource
    }

    /// A person, available from `start`.
    pub fn employee(&mut self, id: &str, name: &str, start: String) -> &mut Resource {
        self.resource(id, name, ResourceKind::Employee, start)
    }

    pub fn add(&mut self, task: Task) -> &mut Task {
        self.plan.tasks.push(task);
        self.plan.tasks.last_mut().expect("just pushed")
    }

    pub fn finish(self) -> Plan {
        self.plan
    }
}

/// A tag held always.
pub fn holds(tag: &str) -> TagRef {
    TagRef {
        tag: tag.to_string(),
        from: None,
        until: None,
    }
}

/// A tag held only inside `[from, until)`.
pub fn holds_between(tag: &str, from: String, until: String) -> TagRef {
    TagRef {
        tag: tag.to_string(),
        from: Some(from),
        until: Some(until),
    }
}

/// An unavailable span with a label.
pub fn off(start: String, end: String, label: &str) -> Span {
    Span {
        start,
        end,
        label: Some(label.to_string()),
    }
}

/// A duration on one resource.
pub fn on(resource: &str, minutes: i64) -> DurationOn {
    DurationOn {
        resource: resource.to_string(),
        duration: minutes,
    }
}

/// A placement of `task` on a resource, with the start it keeps if pinned.
pub fn placed(task: &str, start: Option<String>) -> Assignment {
    Assignment {
        task: task.to_string(),
        start,
    }
}

/// Unavailable spans outside daily working hours `from`–`to` (hours) on
/// days `0..days`, plus the whole weekend days in that range.
pub fn outside_hours(days: i64, from: i64, to: i64, label: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for day in 0..days {
        if day % 7 >= 5 {
            continue;
        }
        let next_working = if day % 7 == 4 { day + 3 } else { day + 1 };
        spans.push(off(at(day, to, 0), at(next_working, from, 0), label));
    }
    spans
}
