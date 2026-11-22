use crownos_disks::model::{Partition, Volume};

const FSTAB: &str = "/etc/fstab";

/// The device column of `/etc/fstab`: volumes the administrator mounts themselves.
#[derive(Debug, Default)]
pub struct FstabSources(Vec<String>);

impl FstabSources {
    pub fn read() -> Self {
        std::fs::read_to_string(FSTAB)
            .map(|table| Self::parse(&table))
            .unwrap_or_default()
    }

    pub fn parse(table: &str) -> Self {
        let sources = table
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .filter(|source| !source.starts_with('#'))
            .map(str::to_owned)
            .collect();
        Self(sources)
    }

    pub fn lists(&self, partition: &Partition, volume: &Volume) -> bool {
        let tags = [
            ("UUID=", volume.uuid.as_str()),
            ("PARTUUID=", partition.uuid.as_str()),
            ("LABEL=", volume.label.as_str()),
        ];
        self.0.iter().any(|source| {
            *source == partition.device
                || tags.iter().any(|(tag, value)| {
                    !value.is_empty()
                        && source
                            .strip_prefix(tag)
                            .is_some_and(|named| named.eq_ignore_ascii_case(value))
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_the_device_column_of_entries() {
        let fstab = FstabSources::parse(
            "# /dev/sda1 is commented out\n\nUUID=abcd / btrfs rw 0 0\n\t/dev/sdb1  /data ext4 defaults 0 2\n",
        );
        assert_eq!(fstab.0, ["UUID=abcd", "/dev/sdb1"]);
    }
}
