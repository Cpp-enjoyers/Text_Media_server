#[cfg(test)]
mod request_tests {
    use std::{fs::read, time::Duration, vec};

    use common::{
        slc_commands::ServerType,
        web_messages::{
            Compression, RequestMessage, ResponseMessage, Serializable, SerializableSerde,
        },
    };
    use compression::{huffman::HuffmanCompressor, lzw::LZWCompressor, Compressor};
    use serde::{de::DeserializeOwned, Serialize};
    use wg_2024::{
        network::SourceRoutingHeader,
        packet::{Fragment, PacketType},
    };

    use crate::{
        servers::{
            requests_handling::list_dir,
            serialization::fragment_response,
            test_utils::{get_dummy_server_media, get_dummy_server_text},
            NetworkGraph, RequestHandler, ServerType as ST, INITIAL_PDR, MEDIA_PATH, TEXT_PATH,
        },
        GenericServer,
    };

    fn test_handle_request<T: ST, U: Compressor>(
        mut server: GenericServer<T>,
        mut compressor: U,
        request: RequestMessage,
        response: ResponseMessage,
    ) where
        GenericServer<T>: RequestHandler,
        U::Compressed: Serialize + DeserializeOwned,
    {
        let (ds, dr) = crossbeam_channel::unbounded();
        server.network_graph = NetworkGraph::from_edges([(0, 1, INITIAL_PDR), (1, 2, INITIAL_PDR)]);
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
        let mut _frags: u64 = 0;
        let mut v: Vec<[u8; 128]> = Vec::new();
        while let Ok(p) = dr.recv_timeout(Duration::from_secs(1)) {
            match p.pack_type {
                PacketType::Ack(_) => acks += 1,
                PacketType::MsgFragment(f) => {
                    v.push(f.data);
                    _frags += 1;
                }
                _ => panic!(),
            }
        }
        assert!(acks == total);
        let v = <U as Compressor>::Compressed::deserialize(v.into_flattened()).unwrap();
        let data: Vec<u8> = compressor.decompress(v).unwrap();
        let resp: ResponseMessage = ResponseMessage::deserialize(data).unwrap();
        // println!("{:?} --- {:?}", resp, response);
        assert!(resp == response);
    }

    #[test]
    fn list_dir_test() {
        let l: Vec<String> = list_dir(TEXT_PATH).unwrap_or_default();
        assert!(l == vec![TEXT_PATH.to_string() + "file.html"]);
    }

    #[test]
    fn test_text_server_handle_type_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage = RequestMessage::new_type_request(1, Compression::LZW);
        let response: ResponseMessage =
            ResponseMessage::new_type_response(0, Compression::LZW, ServerType::FileServer);
        test_handle_request(get_dummy_server_text(), compressor, request, response);
    }

    #[test]
    fn test_text_server_handle_file_list_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage = RequestMessage::new_text_list_request(1, Compression::LZW);
        let response: ResponseMessage = ResponseMessage::new_text_list_response(
            0,
            Compression::LZW,
            list_dir(TEXT_PATH).unwrap(),
        );
        test_handle_request(get_dummy_server_text(), compressor, request, response);
    }

    #[test]
    fn test_text_server_handle_file_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage = RequestMessage::new_text_request(
            1,
            Compression::LZW,
            TEXT_PATH.to_owned() + "file.html",
        );
        let response: ResponseMessage = ResponseMessage::new_text_response(
            0,
            Compression::LZW,
            read(TEXT_PATH.to_owned() + "file.html").unwrap(),
        );
        test_handle_request(get_dummy_server_text(), compressor, request, response);
    }

    #[test]
    fn test_text_server_handle_unknown_file_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage =
            RequestMessage::new_text_request(1, Compression::LZW, "non_esisto".to_string());
        let response: ResponseMessage =
            ResponseMessage::new_not_found_response(0, Compression::LZW);
        test_handle_request(get_dummy_server_text(), compressor, request, response);
    }

    #[test]
    fn test_text_server_handle_media_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage =
            RequestMessage::new_media_request(1, Compression::LZW, "non_esisto".to_string());
        let response: ResponseMessage =
            ResponseMessage::new_invalid_request_response(0, Compression::LZW);
        test_handle_request(get_dummy_server_text(), compressor, request, response);
    }

    #[test]
    fn test_media_server_handle_type_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage = RequestMessage::new_type_request(1, Compression::LZW);
        let response: ResponseMessage =
            ResponseMessage::new_type_response(0, Compression::LZW, ServerType::MediaServer);
        test_handle_request(get_dummy_server_media(), compressor, request, response);
    }

    #[test]
    fn test_media_server_handle_file_list_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage = RequestMessage::new_media_list_request(1, Compression::LZW);
        let response: ResponseMessage = ResponseMessage::new_media_list_response(
            0,
            Compression::LZW,
            list_dir(MEDIA_PATH).unwrap(),
        );
        test_handle_request(get_dummy_server_media(), compressor, request, response);
    }

    #[test]
    fn test_media_server_handle_file_request() {
        let compressor: HuffmanCompressor = HuffmanCompressor::new();
        let request: RequestMessage = RequestMessage::new_media_request(
            1,
            Compression::Huffman,
            MEDIA_PATH.to_owned() + "image.jpg",
        );
        let response: ResponseMessage = ResponseMessage::new_media_response(
            0,
            Compression::Huffman,
            read(MEDIA_PATH.to_owned() + "image.jpg").unwrap(),
        );
        test_handle_request(get_dummy_server_media(), compressor, request, response);
    }

    #[test]
    fn test_media_server_handle_unknown_file_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage =
            RequestMessage::new_media_request(1, Compression::LZW, "non_esisto".to_string());
        let response: ResponseMessage =
            ResponseMessage::new_not_found_response(0, Compression::LZW);
        test_handle_request(get_dummy_server_media(), compressor, request, response);
    }

    #[test]
    fn test_media_server_handle_text_request() {
        let compressor: LZWCompressor = LZWCompressor::new();
        let request: RequestMessage =
            RequestMessage::new_text_request(1, Compression::LZW, "non_esisto".to_string());
        let response: ResponseMessage =
            ResponseMessage::new_invalid_request_response(0, Compression::LZW);
        test_handle_request(get_dummy_server_media(), compressor, request, response);
    }
}
