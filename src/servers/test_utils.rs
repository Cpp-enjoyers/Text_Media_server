use std::collections::HashMap;

use common::Server;

use super::GenericServer;

#[must_use]
pub(super) fn get_dummy_server() -> GenericServer {
    let (ctrl_send, _) = crossbeam_channel::unbounded();
    let (_, ctrl_recv) = crossbeam_channel::unbounded();
    let (_, server_recv) = crossbeam_channel::unbounded();
    let server: GenericServer =
        GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
    server
}
