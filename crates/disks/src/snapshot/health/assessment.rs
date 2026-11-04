use crate::model::{HealthNote, HealthStatus};

/// Accumulates findings: the worst status wins and each note appears once.
pub(super) struct Assessment {
    pub status: HealthStatus,
    pub notes: Vec<HealthNote>,
}

impl Assessment {
    pub const fn new() -> Self {
        Self {
            status: HealthStatus::Good,
            notes: Vec::new(),
        }
    }

    pub fn raise_if(&mut self, condition: bool, status: HealthStatus, note: HealthNote) {
        if !condition {
            return;
        }
        self.status = self.status.max(status);
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }
}
