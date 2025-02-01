use std::collections::HashMap;

use common::Server;

use super::{GenericServer, Media, Text};

#[must_use]
pub(super) fn get_dummy_server_text() -> GenericServer<Text> {
    let (ctrl_send, _) = crossbeam_channel::unbounded();
    let (_, ctrl_recv) = crossbeam_channel::unbounded();
    let (_, server_recv) = crossbeam_channel::unbounded();
    let server: GenericServer<Text> =
        GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
    server
}

#[must_use]
pub(super) fn get_dummy_server_media() -> GenericServer<Media> {
    let (ctrl_send, _) = crossbeam_channel::unbounded();
    let (_, ctrl_recv) = crossbeam_channel::unbounded();
    let (_, server_recv) = crossbeam_channel::unbounded();
    let server: GenericServer<Media> =
        GenericServer::new(0, ctrl_send, ctrl_recv, server_recv, HashMap::new());
    server
}
