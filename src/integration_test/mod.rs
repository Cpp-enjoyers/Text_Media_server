#[cfg(test)]
mod client_interaction_tests {
    #[allow(unused_imports)]
    use std::{
        env,
        fs::read,
        iter::repeat_with,
        thread::{self, sleep},
        time::Duration,
    };

    use ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone;
    use common::{
        slc_commands::{
            ServerCommand, ServerEvent, TextMediaResponse, WebClientCommand, WebClientEvent,
        },
        Client, Server,
    };
    use crossbeam_channel::{Receiver, Sender};
    use rand::{thread_rng, Rng};
    use web_client::web_client::WebBrowser;
    use wg_2024::{
        controller::{DroneCommand, DroneEvent},
        drone::Drone,
        packet::Packet,
    };

    use crate::{
        servers::{Media, Text},
        GenericServer,
    };

    fn instanciate_testing_topology() -> (
        Vec<Sender<DroneCommand>>,
        Receiver<DroneEvent>,
        Sender<ServerCommand>,
        Receiver<ServerEvent>,
        Sender<ServerCommand>,
        Receiver<ServerEvent>,
        Sender<WebClientCommand>,
        Receiver<WebClientEvent>,
    ) {
        // env::set_var("RUST_LOG", "info");
        // let _ = env_logger::try_init();

        let (st_events, st_eventr) = crossbeam_channel::unbounded();
        let (st_ctrls, st_ctrlr) = crossbeam_channel::unbounded();
        let (sm_events, sm_eventr) = crossbeam_channel::unbounded();
        let (sm_ctrls, sm_ctrlr) = crossbeam_channel::unbounded();
        let (c_events, c_eventr) = crossbeam_channel::unbounded();
        let (c_ctrls, c_ctrlr) = crossbeam_channel::unbounded();
        let (d_events, d_eventr) = crossbeam_channel::unbounded();
        let (tservers, tserverr) = crossbeam_channel::unbounded();
        let (smervers, mserverr) = crossbeam_channel::unbounded();
        let (clients, clientr) = crossbeam_channel::unbounded();
        let drone_command: Vec<(Sender<DroneCommand>, Receiver<DroneCommand>)> =
            repeat_with(|| crossbeam_channel::unbounded())
                .take(10)
                .collect();
        let drone_channels: Vec<(Sender<Packet>, Receiver<Packet>)> =
            repeat_with(|| crossbeam_channel::unbounded())
                .take(10)
                .collect();
        let drone_nbrs: [Vec<u8>; 10] = [
            vec![1, 5],
            vec![0, 2, 6],
            vec![1, 3, 7],
            vec![2, 4, 8],
            vec![3, 9],
            vec![0, 6],
            vec![5, 7, 1],
            vec![6, 8, 2],
            vec![7, 9, 3],
            vec![8, 4],
        ];
        for i in 0u8..10u8 {
            let mut map: std::collections::HashMap<u8, Sender<Packet>> = drone_nbrs[i as usize]
                .iter()
                .map(|&id| (id, drone_channels[id as usize].0.clone()))
                .collect();
            if i == 0 || i == 5 {
                map.insert(11, tservers.clone());
                map.insert(13, smervers.clone());
            } else if i == 4 || i == 9 {
                map.insert(12, clients.clone());
            }
            let mut drone: CppEnjoyersDrone = CppEnjoyersDrone::new(
                i,
                d_events.clone(),
                drone_command[i as usize].1.clone(),
                drone_channels[i as usize].1.clone(),
                map,
                // if too high, the test might fail for no reason besides me being unlucky
                Rng::gen_range(&mut thread_rng(), 0., 0.5),
            );
            thread::spawn(move || drone.run());
        }
        let mut server_t: GenericServer<Text> = Server::new(
            11,
            st_events.clone(),
            st_ctrlr.clone(),
            tserverr.clone(),
            [
                (0u8, drone_channels[0].0.clone()),
                (5u8, drone_channels[5].0.clone()),
            ]
            .into_iter()
            .collect(),
        );
        let mut server_m: GenericServer<Media> = Server::new(
            13,
            sm_events.clone(),
            sm_ctrlr.clone(),
            mserverr.clone(),
            [
                (0u8, drone_channels[0].0.clone()),
                (5u8, drone_channels[5].0.clone()),
            ]
            .into_iter()
            .collect(),
        );
        let mut client: WebBrowser = Client::new(
            12,
            c_events.clone(),
            c_ctrlr.clone(),
            clientr.clone(),
            [
                (4u8, drone_channels[4].0.clone()),
                (9u8, drone_channels[9].0.clone()),
            ]
            .into_iter()
            .collect(),
        );
        thread::spawn(move || server_t.run());
        thread::spawn(move || server_m.run());
        thread::spawn(move || client.run());

        (
            drone_command.into_iter().map(|(s, _)| s).collect(),
            d_eventr,
            st_ctrls,
            st_eventr,
            sm_ctrls,
            sm_eventr,
            c_ctrls,
            c_eventr,
        )
    }

    fn generic_full_file_request(
        cevents: Receiver<WebClientEvent>,
        cctrl: Sender<WebClientCommand>,
        file: String,
        check_file: impl Fn(TextMediaResponse) -> (),
    ) {
        sleep(Duration::from_secs(1));
        let _ = cctrl.send(WebClientCommand::AskServersTypes);
        let mut flag: bool = false;
        while let Ok(e) = cevents.recv() {
            match e {
                WebClientEvent::FileFromClient(r, _) => {
                    check_file(r);
                    flag = true;
                    break;
                }
                WebClientEvent::ServersTypes(_) => {
                    let _ = cctrl.send(WebClientCommand::AskListOfFiles(11));
                }
                WebClientEvent::ListOfFiles(_, _) => {
                    let _ = cctrl.send(WebClientCommand::RequestFile(file.clone(), 11));
                }
                WebClientEvent::UnsupportedRequest => {
                    panic!();
                }
                _ => {}
            }
        }
        assert!(flag);
    }

    #[test]
    fn test_full_text_file_request1() {
        let (_dcmds, _devents, _stctrl, _stevents, _smctrl, _smevents, cctrl, cevents) =
            instanciate_testing_topology();
        generic_full_file_request(
            cevents,
            cctrl,
            "./public/file.html".to_owned(),
            |r: TextMediaResponse| {
                assert!(r.get_media_files().is_empty());
                assert!(r.get_html_file().1 == read("./public/file.html").unwrap());
            },
        );
    }

    #[test]
    #[ignore = "computationally expensive"]
    fn test_full_text_file_request2() {
        let (_dcmds, _devents, _stctrl, _stevents, _smctrl, _smevents, cctrl, cevents) =
            instanciate_testing_topology();
        generic_full_file_request(
            cevents,
            cctrl,
            "./public/index.html".to_owned(),
            |r: TextMediaResponse| {
                assert!(r.get_media_files().len() == 1);
                assert!(r.get_media_files()[0].1 == read("./media/rust.png").unwrap());
                assert!(r.get_html_file().1 == read("./public/index.html").unwrap());
            },
        );
    }

    #[test]
    #[ignore = "computationally expensive"]
    fn test_full_text_file_request3() {
        let (_dcmds, _devents, _stctrl, _stevents, _smctrl, _smevents, cctrl, cevents) =
            instanciate_testing_topology();
        generic_full_file_request(
            cevents,
            cctrl,
            "./public/file2.html".to_owned(),
            |r: TextMediaResponse| {
                assert!(r.get_media_files().len() == 3);
                assert!(r.get_media_files()[0].1 == read("./media/rust.png").unwrap());
                assert!(r.get_media_files()[1].1 == read("./media/rust.png").unwrap());
                assert!(r.get_media_files()[2].1 == read("./media/rust.png").unwrap());
                assert!(r.get_html_file().1 == read("./public/file2.html").unwrap());
            },
        );
    }

    #[test]
    #[ignore = "computationally expensive"]
    fn test_full_text_file_request4() {
        let (_dcmds, _devents, _stctrl, _stevents, _smctrl, _smevents, cctrl, cevents) =
            instanciate_testing_topology();
        generic_full_file_request(
            cevents,
            cctrl,
            "./public/three.html".to_owned(),
            |r: TextMediaResponse| {
                assert!(r.get_media_files().len() == 3);
                assert!(r.get_media_files()[0].1 == read("./media/rust.png").unwrap());
                assert!(r.get_media_files()[1].1 == read("./media/c++.png").unwrap());
                assert!(r.get_media_files()[2].1 == read("./media/haskell.jpg").unwrap());
                assert!(r.get_html_file().1 == read("./public/three.html").unwrap());
            },
        );
    }

    #[test]
    #[ignore = "computationally expensive"]
    fn test_after_crashed_drone() {
        // env::set_var("RUST_LOG", "info");
        // let _ = env_logger::try_init();
        let (dcmds, _devents, _stctrl, _stevents, _smctrl, _smevents, cctrl, cevents) =
            instanciate_testing_topology();
        for c in dcmds.iter() {
            let _ = c.send(DroneCommand::SetPacketDropRate(0.));
        }
        let _ = dcmds[0].send(DroneCommand::RemoveSender(1));
        let _ = dcmds[2].send(DroneCommand::RemoveSender(1));
        let _ = dcmds[6].send(DroneCommand::RemoveSender(1));
        let _ = dcmds[1].send(DroneCommand::Crash);
        let _ = dcmds[7].send(DroneCommand::RemoveSender(8));
        let _ = dcmds[9].send(DroneCommand::RemoveSender(8));
        let _ = dcmds[3].send(DroneCommand::RemoveSender(8));
        let _ = dcmds[8].send(DroneCommand::Crash);
        generic_full_file_request(
            cevents,
            cctrl,
            "./public/file.html".to_owned(),
            |r: TextMediaResponse| {
                assert!(r.get_media_files().is_empty());
                assert!(r.get_html_file().1 == read("./public/file.html").unwrap());
            },
        );
    }

    #[test]
    #[ignore = "computationally expensive"]
    fn test_after_removed_drone() {
        let (dcmds, _devents, stctrl, _stevents, _smctrl, _smevents, cctrl, cevents) =
            instanciate_testing_topology();
        let _ = stctrl.send(ServerCommand::RemoveSender(0));
        let _ = dcmds[0].send(DroneCommand::RemoveSender(11));
        generic_full_file_request(
            cevents,
            cctrl,
            "./public/file.html".to_owned(),
            |r: TextMediaResponse| {
                assert!(r.get_media_files().is_empty());
                assert!(r.get_html_file().1 == read("./public/file.html").unwrap());
            },
        );
    }
}
