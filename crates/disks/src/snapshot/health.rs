mod assessment;
mod ata;
mod nvme;

use ata::AtaSmart;
use nvme::{NvmeAttributes, NvmeSmart};

use super::object_view::ObjectView;
use crate::model::Health;
use crate::object_map::{ObjectMap, interface};

const KELVIN_OFFSET: f64 = 273.15;
const SECONDS_PER_HOUR: u64 = 3600;

fn kelvin_to_celsius(kelvin: f64) -> f32 {
    (kelvin - KELVIN_OFFSET) as f32
}

/// Health of a drive from its cached SMART properties; `Unavailable` when the
/// drive has neither an ATA nor an NVMe SMART interface (USB bridges, SD, loop).
pub(super) fn drive_health(map: &ObjectMap, drive: ObjectView<'_>) -> Health {
    if let Some(controller) = drive.properties(interface::NVME_CONTROLLER) {
        let attributes = map
            .nvme_attributes
            .get(drive.path)
            .map(NvmeAttributes::from_properties);
        return NvmeSmart::from_properties(controller, attributes).report();
    }
    drive
        .properties(interface::ATA)
        .map_or(Health::Unavailable, |ata| {
            AtaSmart::from_properties(ata).report()
        })
}
