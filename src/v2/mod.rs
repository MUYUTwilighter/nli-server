//! NetherLink v2 domain and application modules.
//!
//! No HTTP or WebSocket business route is mounted during Phase 0.

/// Indicates that the active tree is still the non-serving Phase 0 bootstrap.
pub const BUSINESS_ROUTES_MOUNTED: usize = 0;

#[cfg(test)]
mod tests {
    use super::BUSINESS_ROUTES_MOUNTED;

    #[test]
    fn phase_zero_mounts_no_business_routes() {
        assert_eq!(BUSINESS_ROUTES_MOUNTED, 0);
    }
}
