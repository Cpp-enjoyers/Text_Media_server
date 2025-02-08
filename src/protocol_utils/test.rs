#[cfg(test)]
mod protocol_tests {
    use crate::protocol_utils::{generate_response_id, get_rid, next_sid, SID_MASK};

    /// tests correct generation of response ids
    #[test]
    fn test_response_id() {
        assert!(generate_response_id(0, 0) == 0);
        assert!(generate_response_id(0, 256) == 256);
        assert!(generate_response_id(1, 23) == u64::from(u16::MAX) + 24);
    }

    /// tests correct generation of request ids
    #[test]
    fn test_get_rid() {
        assert_eq!(get_rid(u64::from(u16::MAX) + 1), 0);
        assert_eq!(get_rid(u64::MAX), u16::MAX);
        assert_eq!(get_rid(u64::from(u16::MAX) + 56), 55);
    }

    /// tests correct generation of session ids
    #[test]
    fn test_next_sid() {
        assert_eq!(next_sid(0), 1);
        assert_eq!(next_sid(SID_MASK), 0);
        assert_eq!(next_sid(42), 43);
    }
}
