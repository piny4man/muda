/// Former hard cap; now the per-file memory warning threshold.
pub const WARN_FILE_BYTES: usize = 50 * 1024 * 1024;
/// Sum of original bytes in the queue that triggers a batch warning.
pub const WARN_QUEUE_BYTES: usize = 256 * 1024 * 1024;

pub const FILE_MEMORY_WARNING: &str = "Large file — stripping may fail on this device.";
pub const QUEUE_MEMORY_WARNING: &str = "This batch may exhaust memory on this device.";

/// Warn when a single original is at or above 50 MiB. Never rejects.
pub fn file_warning(size: usize) -> Option<&'static str> {
    (size >= WARN_FILE_BYTES).then_some(FILE_MEMORY_WARNING)
}

/// Warn when queued originals are at or above 256 MiB. Never rejects.
pub fn queue_warning(total_original_bytes: usize) -> Option<&'static str> {
    (total_original_bytes >= WARN_QUEUE_BYTES).then_some(QUEUE_MEMORY_WARNING)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_under_threshold_is_ok() {
        assert_eq!(file_warning(0), None);
        assert_eq!(file_warning(WARN_FILE_BYTES - 1), None);
    }

    #[test]
    fn file_at_and_over_threshold_warns() {
        assert_eq!(file_warning(WARN_FILE_BYTES), Some(FILE_MEMORY_WARNING));
        assert_eq!(file_warning(WARN_FILE_BYTES + 1), Some(FILE_MEMORY_WARNING));
    }

    #[test]
    fn queue_under_threshold_is_ok() {
        assert_eq!(queue_warning(0), None);
        assert_eq!(queue_warning(WARN_QUEUE_BYTES - 1), None);
    }

    #[test]
    fn queue_at_and_over_threshold_warns() {
        assert_eq!(queue_warning(WARN_QUEUE_BYTES), Some(QUEUE_MEMORY_WARNING));
        assert_eq!(
            queue_warning(WARN_QUEUE_BYTES + 1),
            Some(QUEUE_MEMORY_WARNING)
        );
    }

    #[test]
    fn policy_never_rejects() {
        for size in [0, WARN_FILE_BYTES, usize::MAX] {
            let _ = file_warning(size);
        }
        for total in [0, WARN_QUEUE_BYTES, usize::MAX] {
            let _ = queue_warning(total);
        }
    }

    #[test]
    fn warning_copy_names_memory_and_device() {
        for msg in [FILE_MEMORY_WARNING, QUEUE_MEMORY_WARNING] {
            let lower = msg.to_ascii_lowercase();
            assert!(
                lower.contains("memory") || lower.contains("device"),
                "{msg}"
            );
        }
    }
}
