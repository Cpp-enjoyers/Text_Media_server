#[cfg(test)]
mod serialization_tests {
    use common::web_messages::{
        Compression, RequestMessage, ResponseMessage, Serializable, SerializationError,
    };

    use crate::servers::serialization::{defragment_deserialize_request, fragment_response};

    #[test]
    fn test_fragment1() {
        let resp: ResponseMessage =
            ResponseMessage::new_invalid_request_response(0, Compression::LZW);
        let serialization: Vec<u8> = resp.serialize().unwrap();
        let fragmented: Vec<[u8; 128]> = fragment_response(serialization);
        // let real_sz = (fragmented.len() - 1) * 128 + sz as usize;
        let data: Vec<u8> = fragmented.into_iter().flatten().collect();
        // data.resize(real_sz, 0);
        let resp_d: Result<ResponseMessage, SerializationError> =
            ResponseMessage::deserialize(data);
        assert!(resp_d.is_ok());
        assert_eq!(resp_d.unwrap(), resp);
    }

    #[test]
    fn test_fragment2() {
        let resp: ResponseMessage = ResponseMessage::new_text_list_response(
            0,
            Compression::None,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        let serialization: Vec<u8> = resp.serialize().unwrap();
        let fragmented: Vec<[u8; 128]> = fragment_response(serialization);
        // let real_sz = (fragmented.len() - 1) * 128 + sz as usize;
        let data: Vec<u8> = fragmented.into_iter().flatten().collect();
        // data.resize(real_sz, 0);
        let resp_d: Result<ResponseMessage, SerializationError> =
            ResponseMessage::deserialize(data);
        assert!(resp_d.is_ok());
        assert_eq!(resp_d.unwrap(), resp);
    }

    #[test]
    fn test_fragment3() {
        let file_data: String =
            r#"Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium 
        doloremque laudantium, totam rem aperiam eaque ipsa, quae ab illo inventore 
        veritatis et quasi architecto beatae vitae dicta sunt, explicabo. Nemo enim ipsam 
        voluptatem, quia voluptas sit, aspernatur aut odit aut fugit, sed quia consequuntur 
        magni dolores eos, qui ratione voluptatem sequi nesciunt, neque porro quisquam est, 
        qui dolorem ipsum, quia dolor sit, amet, consectetur, adipisci velit, sed quia non 
        numquam eius modi tempora incidunt, ut labore et dolore magnam aliquam quaerat 
        voluptatem. Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis 
        suscipit laboriosam, nisi ut aliquid ex ea commodi consequatur? Quis autem vel eum 
        jure reprehenderit, qui in ea voluptate velit esse, quam nihil molestiae consequatur, 
        vel illum, qui dolorem eum fugiat, quo voluptas nulla pariatur? [33] At vero eos et 
        accusamus et iusto odio dignissimos ducimus, qui blanditiis praesentium voluptatum 
        deleniti atque corrupti, quos dolores et quas molestias excepturi sint, obcaecati 
        cupiditate non provident, similique sunt in culpa, qui officia deserunt mollitia 
        animi, id est laborum et dolorum fuga. Et harum quidem rerum facilis est et expedita 
        distinctio. Nam libero tempore, cum soluta nobis est eligendi optio, cumque nihil 
        impedit, quo minus id, quod maxime placeat, facere possimus, omnis voluptas 
        assumenda est, omnis dolor repellendus. Temporibus autem quibusdam et aut officiis 
        debitis aut rerum necessitatibus saepe eveniet, ut et voluptates repudiandae sint et 
        mollitia non recusandae. Itaque earum rerum hic tenetur a sapiente delectus, ut aut 
        reiciendis voluptatibus maiores alias consequatur aut perferendis doloribus 
        asperiores repellat."#
                .to_string();

        let resp: ResponseMessage =
            ResponseMessage::new_text_response(0, Compression::None, file_data.as_bytes().to_vec());
        let serialization: Vec<u8> = resp.serialize().unwrap();
        let fragmented: Vec<[u8; 128]> = fragment_response(serialization);
        // let real_sz = (fragmented.len() - 1) * 128 + sz as usize;
        let data: Vec<u8> = fragmented.into_iter().flatten().collect();
        // data.resize(real_sz, 0);
        let resp_d: Result<ResponseMessage, SerializationError> =
            ResponseMessage::deserialize(data);
        assert!(resp_d.is_ok());
        assert_eq!(resp_d.unwrap(), resp);
    }

    #[test]
    fn test_fragment4() {
        let resp: ResponseMessage = ResponseMessage::new_text_list_response(
            0,
            Compression::None,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        let serialization: Vec<u8> = resp.serialize().unwrap();
        let fragmented: Vec<[u8; 128]> = fragment_response(serialization);
        // let real_sz = (fragmented.len() - 1) * 128 + sz as usize;
        let mut data: Vec<u8> = fragmented.into_iter().flatten().collect();
        // data.resize(real_sz, 0);
        data[5] = 123u8; // corrupt data
        let resp_d: Result<ResponseMessage, SerializationError> =
            ResponseMessage::deserialize(data);
        assert!(resp_d.is_err());
    }

    #[test]
    fn test_defragment1() {
        let req: RequestMessage =
            RequestMessage::new_text_request(0, Compression::LZW, "file".to_string());
        let data: Vec<u8> = req.serialize().unwrap();
        let data: Vec<[u8; 128]> = fragment_response(data);
        let req_d: Result<RequestMessage, SerializationError> =
            defragment_deserialize_request(data);
        assert!(req_d.is_ok());
        assert_eq!(req_d.unwrap(), req);
    }

    #[test]
    fn test_defragment2() {
        let req: RequestMessage = RequestMessage::new_type_request(0, Compression::LZW);
        let data: Vec<u8> = req.serialize().unwrap();
        let data: Vec<[u8; 128]> = fragment_response(data);
        let req_d: Result<RequestMessage, SerializationError> =
            defragment_deserialize_request(data);
        assert!(req_d.is_ok());
        assert_eq!(req_d.unwrap(), req);
    }

    #[test]
    fn test_defragment3() {
        let req: RequestMessage = RequestMessage::new_text_list_request(0, Compression::LZW);
        let data: Vec<u8> = req.serialize().unwrap();
        let data: Vec<[u8; 128]> = fragment_response(data);
        let req_d: Result<RequestMessage, SerializationError> =
            defragment_deserialize_request(data);
        assert!(req_d.is_ok());
        assert_eq!(req_d.unwrap(), req);
    }

    #[test]
    fn test_defragment4() {
        let req: RequestMessage = RequestMessage::new_text_list_request(0, Compression::LZW);
        let mut data: Vec<u8> = req.serialize().unwrap();
        data[3] = 57u8; // corrupt data
        let data: Vec<[u8; 128]> = fragment_response(data);
        let req_d: Result<RequestMessage, SerializationError> =
            defragment_deserialize_request(data);
        assert!(req_d.is_err());
        // assert_eq!(req_d.unwrap(), req);
    }
}
