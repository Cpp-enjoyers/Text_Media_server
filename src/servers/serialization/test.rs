#[cfg(test)]
mod tests {
    use common::web_messages::{Compression, ResponseMessage, Serializable};

    use crate::servers::serialization::fragment_response;

    #[test]
    fn test1() {
        let resp: ResponseMessage =
            ResponseMessage::new_invalid_request_response(0, Compression::LZW);
        let serialization: Vec<u8> = resp.serialize().unwrap();
        println!("{serialization:?}");
        let fragmented: Vec<[u8; 128]> = fragment_response(serialization);
        // let real_sz = (fragmented.len() - 1) * 128 + sz as usize;
        let data: Vec<u8> = fragmented.into_iter().flatten().collect();
        println!("{data:?}");
        // data.resize(real_sz, 0);
        let resp_d = ResponseMessage::deserialize(data);
        assert!(resp_d.is_ok());
        // assert_eq!(resp_d.unwrap(), resp);
    }
}
