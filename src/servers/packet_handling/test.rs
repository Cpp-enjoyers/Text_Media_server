#[cfg(test)]
mod tests {
    use wg_2024::packet::{Ack, Nack, NackType, FRAGMENT_DSIZE};

    use crate::{servers::test_utils::get_dummy_server, GenericServer};

    #[test]
    fn test_get_rid() {
        assert_eq!(GenericServer::get_rid(u64::from(u16::MAX) + 1), 0);
        assert_eq!(GenericServer::get_rid(u64::MAX), u16::MAX);
        assert_eq!(GenericServer::get_rid(u64::from(u16::MAX) + 56), 55);
    }

    #[test]
    fn test_ack() {
        let mut server: GenericServer = get_dummy_server();
        let ack: Ack = Ack { fragment_index: 0 };
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        server.handle_ack(0, &ack);
        assert!(server.sent_history.is_empty());
    }

    #[test]
    fn test_ack_missing() {
        let mut server: GenericServer = get_dummy_server();
        let ack: Ack = Ack { fragment_index: 0 };
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        server.handle_ack(1, &ack);
        assert!(server.sent_history.len() == 1);
    }

    #[test]
    fn test_nack_to_pending() {
        let mut server: GenericServer = get_dummy_server();
        server
            .sent_history
            .insert(0, (1, 0, 1, [0; FRAGMENT_DSIZE]));
        let nack: Nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        server.handle_nack(0, &nack);
        assert_eq!(server.pending_packets.pop_back().unwrap(), 0);
    }

    #[test]
    fn test_nack_resend() {
        todo!();
    }
}
