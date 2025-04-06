use doip_rw_tokio::DoIpUdpConnection;
use scenario::parser::Step;
use std::io::{self};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{self, Duration};

mod argparse;
mod scenario;

async fn discover_doip_entities(
    local_addr: SocketAddr,
    broadcast_addr: SocketAddr,
) -> io::Result<()> {
    let socket = UdpSocket::bind(local_addr).await?;
    socket.set_broadcast(true)?;
    let mut doip_udp = DoIpUdpConnection::new(socket);
    doip_udp
        .send_vehicle_identification_request(broadcast_addr)
        .await
        .map_err(|e| match e {
            doip_rw_tokio::DoIpCnxError::ConnectionError(io) => io,
            _ => io::Error::new(io::ErrorKind::InvalidData, ""),
        })?;
    println!("Broadcasting: Vehicle Identification Request !");

    let sleep = time::sleep(Duration::from_millis(4000));
    tokio::pin!(sleep);
    loop {
        #[rustfmt::skip]
        tokio::select! {
            _ = &mut sleep => {
		println!("Time: finished waiting for Vehicle Identification Responses");
		break;
            },
            Ok(msg) = doip_udp.receive_message() => {
                println!("UDP Received: {:?}", msg);
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    use log::LevelFilter;
    let args = argparse::get_args();

    if let Some(commands) = &args.uds_commands {
        if args.uds_commands.is_some() && !commands.is_empty() {
            env_logger::builder()
                .filter_module("uds", LevelFilter::Trace)
                .init();
        } else {
            env_logger::init();
        }
    } else {
        env_logger::init();
    }

    if args.discover {
        discover_doip_entities(args.local_addr, args.broadcast_addr)
            .await
            .unwrap();
    }

    let mut steps = vec![];
    if let Some(commands) = args.uds_commands {
        if !commands.is_empty() {
            for command in commands {
                steps.push(Step::RawUds(scenario::parser::RawUds {
                    uds_bytes: command,
                }));
            }
        }
    }

    if let Some(scenario_filenames) = args.scenario {
        for scenario_filename in scenario_filenames.split(',') {
            let mut this_steps = scenario::parser::read_scenario(scenario_filename);
            steps.append(&mut this_steps);
        }
    }

    scenario::main::scenario(
        args.local_addr,
        args.remote_addr,
        args.doip_la,
        args.doip_ta,
        steps,
    )
    .await
    .unwrap_or_else(|err| {
        panic!("Scenario aborted due to an error: {err}");
    });
}
