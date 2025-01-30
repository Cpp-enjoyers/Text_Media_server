#[cfg(test)]
mod request_tests {
    use std::{fs::read_to_string, time::Duration, vec};

    use common::{
        slc_commands::ServerType,
        web_messages::{Compression, RequestMessage, ResponseMessage, Serializable},
    };
    use compression::{lzw::LZWCompressor, Compressor};
    use wg_2024::{
        network::SourceRoutingHeader,
        packet::{Fragment, PacketType},
    };

    use crate::{
        servers::{serialization::fragment_response, test_utils::get_dummy_server, NetworkGraph},
        GenericServer,
    };

    fn test_handle_request(request: RequestMessage, response: ResponseMessage) {
        assert!(request.compression_type == Compression::LZW);
        let mut server: GenericServer = get_dummy_server();
        let (ds, dr) = crossbeam_channel::unbounded();
        server.network_graph = NetworkGraph::from_edges([(0, 1, 1.), (1, 2, 1.)]);
        server.packet_send.insert(1, ds);
        let data: Vec<[u8; 128]> = fragment_response(request.serialize().unwrap());
        let total: u64 = u64::try_from(data.len()).unwrap();
        for (i, frag) in data.into_iter().enumerate() {
            server.handle_fragment(
                &SourceRoutingHeader::new(vec![2, 1, 0], 2),
                0,
                &Fragment {
                    fragment_index: i as u64,
                    total_n_fragments: total,
                    length: 128,
                    data: frag,
                },
            );
        }
        assert!(server.fragment_history.is_empty());
        assert!(!server.sent_history.is_empty());
        let mut acks: u64 = 0;
        let mut frags: u64 = 0;
        let mut v: Vec<[u8; 128]> = Vec::new();
        while let Ok(p) = dr.recv_timeout(Duration::from_secs(1)) {
            match p.pack_type {
                PacketType::Ack(_) => acks += 1,
                PacketType::MsgFragment(f) => {
                    v.push(f.data);
                    frags += 1;
                }
                _ => panic!(),
            }
        }
        assert!(acks == frags);
        let v: Vec<u16> = Vec::deserialize(v.into_flattened()).unwrap();
        let data: Vec<u8> = LZWCompressor::new().decompress(v).unwrap();
        let resp: ResponseMessage = ResponseMessage::deserialize(data).unwrap();
        // println!("{:?} --- {:?}", resp, response);
        assert!(resp == response);
    }

    #[test]
    fn test_response_id() {
        assert!(GenericServer::generate_response_id(0, 0) == 0);
        assert!(GenericServer::generate_response_id(0, 256) == 256);
        assert!(GenericServer::generate_response_id(1, 23) == u64::from(u16::MAX) + 24);
    }

    #[test]
    fn list_dir_test() {
        let l: Vec<String> = GenericServer::list_dir("./public/").unwrap_or_default();
        assert!(l == vec!["./public/file.txt".to_string()]);
    }

    #[test]
    fn test_handle_type_request() {
        let request: RequestMessage = RequestMessage::new_type_request(1, Compression::LZW);
        let response: ResponseMessage =
            ResponseMessage::new_type_response(0, Compression::LZW, ServerType::FileServer);
        test_handle_request(request, response);
    }

    #[test]
    fn test_handle_file_list_request() {
        let request: RequestMessage = RequestMessage::new_text_list_request(1, Compression::LZW);
        let response: ResponseMessage = ResponseMessage::new_text_list_response(
            0,
            Compression::LZW,
            GenericServer::list_dir("./public/").unwrap(),
        );
        test_handle_request(request, response);
    }

    #[test]
    fn test_handle_file_request() {
        let request: RequestMessage =
            RequestMessage::new_text_request(1, Compression::LZW, "./public/file.txt".to_string());
        let response: ResponseMessage = ResponseMessage::new_text_response(
            0,
            Compression::LZW,
            read_to_string("./public/file.txt").unwrap(),
        );
        test_handle_request(request, response);
    }

    #[test]
    fn test_handle_unknown_file_request() {
        let request: RequestMessage =
            RequestMessage::new_text_request(1, Compression::LZW, "non_esisto".to_string());
        let response: ResponseMessage =
            ResponseMessage::new_not_found_response(0, Compression::LZW);
        test_handle_request(request, response);
    }

    #[test]
    fn test_handle_media_request() {
        let request: RequestMessage =
            RequestMessage::new_media_request(1, Compression::LZW, "non_esisto".to_string());
        let response: ResponseMessage =
            ResponseMessage::new_invalid_request_response(0, Compression::LZW);
        test_handle_request(request, response);
    }
}
