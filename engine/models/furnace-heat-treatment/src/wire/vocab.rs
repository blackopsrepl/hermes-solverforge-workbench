//! The words a dataset uses: processes, priorities, roles, skills, shifts.

use serde::{Deserialize, Serialize};

/// A heat treatment process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Process {
    Quenching,
    Tempering,
    Annealing,
    Carburizing,
    Nitriding,
    StressRelieving,
    Normalizing,
    SolutionTreating,
    Aging,
}

impl Process {
    pub const ALL: [Process; 9] = [
        Process::Quenching,
        Process::Tempering,
        Process::Annealing,
        Process::Carburizing,
        Process::Nitriding,
        Process::StressRelieving,
        Process::Normalizing,
        Process::SolutionTreating,
        Process::Aging,
    ];

    /// Bit of the process in a furnace's process mask.
    pub const fn bit(self) -> u16 {
        1 << self as u16
    }

    /// Processes that end in a quench unless the recipe says otherwise.
    pub const fn quenches(self) -> bool {
        matches!(
            self,
            Process::Quenching | Process::Carburizing | Process::SolutionTreating
        )
    }

    /// Long diffusion cycles that run through the night by design.
    pub const fn runs_overnight(self) -> bool {
        matches!(self, Process::Carburizing | Process::Nitriding)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Process::Quenching => "Hardening",
            Process::Tempering => "Tempering",
            Process::Annealing => "Annealing",
            Process::Carburizing => "Carburizing",
            Process::Nitriding => "Nitriding",
            Process::StressRelieving => "Stress relieving",
            Process::Normalizing => "Normalizing",
            Process::SolutionTreating => "Solution treating",
            Process::Aging => "Aging",
        }
    }

    /// Extra minutes to change the furnace atmosphere between two processes.
    pub fn family_minutes(from: Process, to: Process) -> i64 {
        use Process::*;
        match (from, to) {
            (a, b) if a == b => 0,
            (Carburizing, Quenching) | (Quenching, Carburizing) => 35,
            (Quenching, Tempering) | (Tempering, Quenching) => 15,
            (SolutionTreating, Aging) | (Aging, SolutionTreating) => 10,
            _ => 20,
        }
    }
}

/// How urgent a work order is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Priority {
    #[default]
    Standard,
    Urgent,
    Express,
}

impl Priority {
    /// Medium score of leaving an order unscheduled.
    pub const fn weight(self) -> i64 {
        match self {
            Priority::Standard => 1,
            Priority::Urgent => 4,
            Priority::Express => 9,
        }
    }

    /// Soft score per minute late.
    pub const fn lateness_per_minute(self) -> i64 {
        match self {
            Priority::Standard => 5,
            Priority::Urgent => 20,
            Priority::Express => 60,
        }
    }

    /// Soft score of a night start for a day process.
    pub const fn night_start_cost(self) -> i64 {
        match self {
            Priority::Standard => 120,
            Priority::Urgent => 70,
            Priority::Express => 40,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Priority::Standard => "standard",
            Priority::Urgent => "urgent",
            Priority::Express => "express",
        }
    }
}

/// An operator's job on the shop floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    ShiftLead,
    FurnaceOperator,
    MaintenanceTechnician,
    MaterialHandler,
    QualityControl,
}

impl Role {
    /// Skills every holder of the role has.
    pub fn base_skills(self) -> u16 {
        use Skill::*;
        let skills: &[Skill] = match self {
            Role::ShiftLead => &[FurnaceProgram, LoadBuild, QuenchOperate, Monitor],
            Role::FurnaceOperator => &[LoadBuild, CraneForklift, QuenchOperate, Monitor],
            Role::MaintenanceTechnician => &[Electrical, Mechanical, Monitor],
            Role::MaterialHandler => &[CraneForklift, ReceiveStage, ShipStore],
            Role::QualityControl => &[QcInspect],
        };
        skills.iter().fold(0, |mask, skill| mask | skill.bit())
    }

    /// Whether the role is ever rostered on nights.
    pub const fn works_nights(self) -> bool {
        !matches!(self, Role::MaterialHandler | Role::QualityControl)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Role::ShiftLead => "Shift lead",
            Role::FurnaceOperator => "Furnace operator",
            Role::MaintenanceTechnician => "Maintenance technician",
            Role::MaterialHandler => "Material handler",
            Role::QualityControl => "Quality control",
        }
    }
}

/// A skill an operator holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Skill {
    FurnaceProgram,
    LoadBuild,
    CraneForklift,
    QuenchOperate,
    Monitor,
    Electrical,
    Mechanical,
    QcInspect,
    ReceiveStage,
    ShipStore,
}

impl Skill {
    pub const fn bit(self) -> u16 {
        1 << self as u16
    }

    pub const fn label(self) -> &'static str {
        match self {
            Skill::FurnaceProgram => "furnace programming",
            Skill::LoadBuild => "load building",
            Skill::CraneForklift => "crane and forklift",
            Skill::QuenchOperate => "quench operation",
            Skill::Monitor => "furnace monitoring",
            Skill::Electrical => "electrical",
            Skill::Mechanical => "mechanical",
            Skill::QcInspect => "quality inspection",
            Skill::ReceiveStage => "receiving",
            Skill::ShipStore => "shipping",
        }
    }
}

/// One of the three eight-hour shifts of a roster day, or a day off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShiftKind {
    Morning,
    Afternoon,
    Night,
    Off,
}

impl ShiftKind {
    /// The roster variable's value for this kind.
    pub const fn value(self) -> usize {
        self as usize
    }

    pub const fn from_value(value: usize) -> Option<ShiftKind> {
        match value {
            0 => Some(ShiftKind::Morning),
            1 => Some(ShiftKind::Afternoon),
            2 => Some(ShiftKind::Night),
            3 => Some(ShiftKind::Off),
            _ => None,
        }
    }

    pub const fn is_working(self) -> bool {
        !matches!(self, ShiftKind::Off)
    }

    pub const fn label(self) -> &'static str {
        match self {
            ShiftKind::Morning => "morning",
            ShiftKind::Afternoon => "afternoon",
            ShiftKind::Night => "night",
            ShiftKind::Off => "off",
        }
    }
}

/// The construction of a furnace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FurnaceKind {
    Chamber,
    Pit,
    CarHearth,
    Aluminum,
    Vacuum,
}

impl FurnaceKind {
    pub const fn label(self) -> &'static str {
        match self {
            FurnaceKind::Chamber => "chamber",
            FurnaceKind::Pit => "pit",
            FurnaceKind::CarHearth => "car hearth",
            FurnaceKind::Aluminum => "aluminium",
            FurnaceKind::Vacuum => "vacuum",
        }
    }
}
