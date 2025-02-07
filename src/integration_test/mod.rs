#[cfg(test)]
mod client_interaction_tests {
    use std::{env, fs::read, iter::repeat, thread::{self, sleep}, time::Duration};

    use ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone;
    use common::{
        slc_commands::{ServerCommand, ServerEvent, WebClientCommand, WebClientEvent},
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
        servers::{RequestHandler, ServerType, Text},
        GenericServer,
    };

    fn instanciate_testing_topology<T: 'static + ServerType + Send>() -> (
        Vec<Sender<DroneCommand>>,
        Receiver<DroneEvent>,
        Sender<ServerCommand>,
        Receiver<ServerEvent>,
        Sender<WebClientCommand>,
        Receiver<WebClientEvent>,
    )
    where
        GenericServer<T>: RequestHandler,
    {
        env::set_var("RUST_LOG", "info");
        let _ = env_logger::try_init();

        let (s_events, s_eventr) = crossbeam_channel::unbounded();
        let (s_ctrls, s_ctrlr) = crossbeam_channel::unbounded();
        let (c_events, c_eventr) = crossbeam_channel::unbounded();
        let (c_ctrls, c_ctrlr) = crossbeam_channel::unbounded();
        let (d_events, d_eventr) = crossbeam_channel::unbounded();
        let (servers, serverr) = crossbeam_channel::unbounded();
        let (clients, clientr) = crossbeam_channel::unbounded();
        let drone_command: Vec<(Sender<DroneCommand>, Receiver<DroneCommand>)> =
            repeat(crossbeam_channel::unbounded()).take(10).collect();
        let drone_channels: Vec<(Sender<Packet>, Receiver<Packet>)> =
            repeat(crossbeam_channel::unbounded()).take(10).collect();
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
                map.insert(11, servers.clone());
            } else if i == 4 || i == 9 {
                map.insert(12, clients.clone());
            }
            println!("{i}: {map:?}");
            let mut drone: CppEnjoyersDrone = CppEnjoyersDrone::new(
                i,
                d_events.clone(),
                drone_command[i as usize].1.clone(),
                drone_channels[i as usize].1.clone(),
                map,
                Rng::gen_range(&mut thread_rng(), 0., 1.),
            );
            thread::spawn(move || drone.run());
        }
        let mut server: GenericServer<T> = Server::new(
            11,
            s_events.clone(),
            s_ctrlr.clone(),
            serverr.clone(),
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
        thread::spawn(move || server.run());
        thread::spawn(move || client.run());

        (
            drone_command.into_iter().map(|(s, _)| s).collect(),
            d_eventr,
            s_ctrls,
            s_eventr,
            c_ctrls,
            c_eventr,
        )
    }

    #[test]
    fn test_full_text_file_request() {
        let (_dcmds, _devents, _sctrl, _sevents, cctrl, cevents) =
            instanciate_testing_topology::<Text>();
        let _ = cctrl.send(WebClientCommand::AskServersTypes);
        sleep(Duration::from_secs(1));
        let _ = cctrl.send(WebClientCommand::AskListOfFiles(11));
        sleep(Duration::from_secs(1));
        let _ = cctrl.send(WebClientCommand::RequestFile(
            "./public/file.html".to_owned(),
            11,
        ));
        sleep(Duration::from_secs(1));
        let mut flag: bool = false;
        let expected_data: Vec<u8> = read("./public/file.html").unwrap();
        while let Ok(e) = cevents.recv_timeout(Duration::from_secs(1)) {
            match e {
                WebClientEvent::FileFromClient(r, _) => {
                    assert!(r.get_media_files().is_empty());
                    assert!(r.get_html_file().1 == expected_data);
                    flag = true;
                }
                _ => {}
            }
        }
        assert!(flag);
    }
}
