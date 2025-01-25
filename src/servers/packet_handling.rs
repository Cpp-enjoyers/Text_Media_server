use std::vec;

use log::{error, info, warn};
use wg_2024::{
    network::SourceRoutingHeader,
    packet::{Ack, FloodResponse, Fragment, Nack, FRAGMENT_DSIZE},
};

use super::GenericServer;

impl GenericServer {
    #[inline]
    fn get_rid(sid: u64) -> u16 {
        // intentional, if shifted by 48 it fits into 16
        u16::try_from(sid << 48).unwrap()
    }

    pub(crate) fn handle_ack(&mut self, sid: u64, _ack: &Ack) {
        self.sent_history.remove(&sid).map_or_else(
            || warn!("Received unknow sid in Ack msg: {sid}"),
            |_| info!("Sid: {sid} acknoledged"),
        );
    }

    pub(crate) fn handle_nack(&mut self, sid: u64, nack: &Nack) {
        if let Some(f) = self.sent_history.get(&sid) {
            todo!();
        } else {
            warn!("Received Nack with unknown sid: {sid}");
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn handle_fragment(
        &mut self,
        srch: &SourceRoutingHeader,
        sid: u64,
        frag: &Fragment,
    ) {
        let rid: u16 = Self::get_rid(sid);
        if let Some(&id) = srch.hops.first() {
            let entry: &mut (u64, Vec<[u8; 128]>) =
                self.fragment_history.entry((id, rid)).or_insert((
                    0,
                    // should be fine on 64 bit machines
                    vec![[0; FRAGMENT_DSIZE]; frag.total_n_fragments as usize],
                ));
            entry.1.get_mut(frag.fragment_index as usize).map_or_else(
                || warn!("Received fragment with invalid index"),
                |v: &mut [u8; 128]| {
                    entry.0 += 1;
                    *v = frag.data;
                },
            );
            if entry.0 == frag.total_n_fragments {
                todo!();
            }
            todo!(); // send back Ack
        } else {
            error!("Received fragment with invalid source routing header!");
        }
    }

    pub(crate) fn handle_flood_response(&mut self, fr: &FloodResponse) {}
}
