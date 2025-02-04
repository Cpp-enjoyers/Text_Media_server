#[cfg(test)]
mod command_tests {
    use common::slc_commands::ServerCommand;

    use crate::{
        servers::{test_utils::get_dummy_server_text, NetworkGraph, Text, INITIAL_PDR},
        GenericServer,
    };

    #[test]
    fn test_add_command() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let (ds, _) = crossbeam_channel::unbounded();
        let command: ServerCommand = ServerCommand::AddSender(1, ds.clone());
        server.handle_command(command);
        assert!(server.packet_send.len() == 1);
        assert!(server.network_graph.contains_node(1));
        assert!(server.need_flood);
    }

    #[test]
    fn test_remove_command() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server.network_graph = NetworkGraph::from_edges([(0, 1, INITIAL_PDR), (1, 2, INITIAL_PDR)]);
        let (ds, _) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        let command: ServerCommand = ServerCommand::RemoveSender(1);
        server.handle_command(command);
        assert!(server.packet_send.is_empty());
        assert!(!server.network_graph.contains_node(1));
        assert!(server.need_flood);
    }
}
