#[cfg(test)]
mod command_tests {
    use common::slc_commands::ServerCommand;

    use crate::{
        servers::{
            self, routing::RoutingTable, test_utils::get_dummy_server_text, NetworkGraph, Text,
            INITIAL_PDR,
        },
        GenericServer,
    };

    /// tests the correct handling of the [ServerCommand::AddSender]
    #[test]
    fn test_add_command() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        let (ds, _) = crossbeam_channel::unbounded();
        let command: ServerCommand = ServerCommand::AddSender(1, ds.clone());
        server.handle_command(command);
        assert!(server.packet_send.len() == 1);
        assert!(server.network_graph.get_graph().contains_node(1));
        assert!(server.network_graph.get_graph().edge_count() == 1);
        assert!(server.need_flood);
    }

    /// tests the correct handling of the [ServerCommand::RemoveSender]
    #[test]
    fn test_remove_command() {
        let mut server: GenericServer<Text> = get_dummy_server_text();
        server.network_graph = RoutingTable::new_with_graph(
            NetworkGraph::from_edges([(0, 1, INITIAL_PDR), (1, 2, INITIAL_PDR)]),
            servers::default_estimator(),
        );
        let (ds, _) = crossbeam_channel::unbounded();
        server.packet_send.insert(1, ds.clone());
        let command: ServerCommand = ServerCommand::RemoveSender(1);
        server.handle_command(command);
        assert!(server.packet_send.is_empty());
        assert!(!server.network_graph.get_graph().contains_node(1));
        assert!(server.need_flood);
    }
}
