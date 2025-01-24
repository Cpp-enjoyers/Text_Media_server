use log::{info, warn};
use wg_2024::packet::Ack;

use super::GenericServer;

impl GenericServer {
    pub(crate) fn handle_ack(&mut self, sid: u64, _ack: &Ack) {
        if let Some((sz, _)) = self.sent_history.get_mut(&sid) {
            info!("Received Ack for fragment: {} - session_id: {sid}", _ack.fragment_index);
            *sz -= 1;
            if *sz == 0 {
                self.sent_history.remove(&sid);
                info!("Message fully acknowledged, removing from history");
            }
        } else {
            warn!("Received Ack with unknown session_id: {sid}");
        }
    }

    // pub(crate) fn handle_nack(&mut self, sid: u64)

}