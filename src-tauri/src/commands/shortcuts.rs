//! Keyboard-shortcut dispatch: pure index helpers, the injected listener JS,
//! and the `handle_shortcut` command. Built up across tasks 1, 3, 4, 6.

/// Cycle direction for [`cycle_index`].
pub enum CycleDir {
    Next,
    Prev,
}

/// Wrapping index for cycling apps. `current` is the active app's index; returns
/// the next/previous index, wrapping. `None` if the list is empty.
pub fn cycle_index(current: usize, len: usize, dir: CycleDir) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match dir {
        CycleDir::Next => (current + 1) % len,
        CycleDir::Prev => (current + len - 1) % len,
    })
}

/// 1-based positional jump → 0-based index. `None` for 0 or past the end. No wrap.
pub fn jump_index(n: usize, len: usize) -> Option<usize> {
    if n == 0 || n > len {
        None
    } else {
        Some(n - 1)
    }
}

/// After removing the app at `removed` from a list of `len_before`, which
/// surviving index to activate? Prefers the app that was after (it slides into
/// `removed`'s slot); else the new last; else `None` (was the only app). No wrap.
pub fn neighbor_index(removed: usize, len_before: usize) -> Option<usize> {
    if len_before <= 1 {
        return None;
    }
    if removed + 1 < len_before {
        Some(removed)
    } else {
        Some(removed - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_next_wraps() {
        assert_eq!(cycle_index(0, 3, CycleDir::Next), Some(1));
        assert_eq!(cycle_index(2, 3, CycleDir::Next), Some(0));
    }

    #[test]
    fn cycle_prev_wraps() {
        assert_eq!(cycle_index(1, 3, CycleDir::Prev), Some(0));
        assert_eq!(cycle_index(0, 3, CycleDir::Prev), Some(2));
    }

    #[test]
    fn cycle_empty_returns_none() {
        assert_eq!(cycle_index(0, 0, CycleDir::Next), None);
        assert_eq!(cycle_index(0, 0, CycleDir::Prev), None);
    }

    #[test]
    fn jump_one_based_no_wrap() {
        assert_eq!(jump_index(1, 3), Some(0));
        assert_eq!(jump_index(3, 3), Some(2));
        assert_eq!(jump_index(4, 3), None);
        assert_eq!(jump_index(0, 3), None);
    }

    #[test]
    fn neighbor_prefers_after_then_before() {
        assert_eq!(neighbor_index(1, 3), Some(1)); // middle: after slides in
        assert_eq!(neighbor_index(2, 3), Some(1)); // last: new last
        assert_eq!(neighbor_index(0, 3), Some(0)); // first of many: after slides in
    }

    #[test]
    fn neighbor_only_app_returns_none() {
        assert_eq!(neighbor_index(0, 1), None);
        assert_eq!(neighbor_index(0, 0), None);
    }
}
