use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::{io, str::from_utf8, time::Duration};

extern crate doip_rw_tokio;

use doip_rw::message::{ActivationType, UdsBuffer};
use doip_rw_tokio::DoIpTcpMessage;
use log::error;
use regex::Regex;
use tokio::{net::TcpListener, task};

use doip_rw_tokio::{DoIpTcpConnection, Timings};

const UDS_ANSWERS: [(&str, &str); 7] = [
    (
        r"22f012",
        "62 f0 12 32 36 34 31 33 30 30 35 30 30 52 31", //"62140350001R"
    ),
    (
        r"22f190",
        "62 f1 90 56 46 31 58 52 32 31 30 46 53 54 47 42 45 4e 30 34", // VF1XR210FSTGBEN04
    ),
    (r"22.*", "7f2210"),
    (r"19 0a", "59 0a ff ea 19 88 00 fd 01 50"),
    // Transfer file:
    (r"34.*", "74 20 0f fa"),
    (r"36.*", "76 01"),
    (r"37.*", "77"),
];

fn print_uds_request(prefix: &str, req: &[u8]) {
    log::info!("{}Uds hexdump: {:02x?}", prefix, req);
}

fn find_uds_answer(req: &[u8]) -> Option<Vec<u8>> {
    let req_nibbles = bin2nibbles(req);
    for (regex, answer) in UDS_ANSWERS.into_iter() {
        let regex = regex.replace(' ', "");
        let re = Regex::new(&regex).unwrap();
        if re.is_match(&req_nibbles) {
            return Some(nibbles2bin(answer));
        }
    }
    None
}

fn bin2nibbles(req: &[u8]) -> String {
    req.iter().fold(String::new(), |mut output, b| {
        let _ = write!(output, "{b:02x}");
        output
    })
}

fn nibbles2bin(req: &str) -> Vec<u8> {
    let req = req.replace(' ', "");
    let nibble2bin = |c| u8::from_str_radix(from_utf8(c).unwrap(), 16).unwrap();
    req.as_bytes()
        .chunks(2)
        .map(nibble2bin)
        .collect::<Vec<u8>>()
}

async fn handle_diag_request(
    client: &mut DoIpTcpConnection,
    req: doip_rw::message::DiagnosticMessage<'_>,
) -> bool {
    let uds = req.user_data.get_ref();
    client
        .send_diagnostic_acknowledge(req.source_address)
        .await
        .unwrap();
    print_uds_request("UDS  input: ", uds);
    let answer = match find_uds_answer(uds) {
        None => vec![0x7f, uds[0], 0x11],
        Some(answer) => answer,
    };
    print_uds_request("UDS output: ", &answer);
    client
        .send_diagnostic_request(req.source_address, UdsBuffer::Owned(answer))
        .await
        .unwrap();
    false
}

async fn handle_cnx(mut client: DoIpTcpConnection, uds_received: Arc<Mutex<Vec<Vec<u8>>>>) {
    let mut is_last = false;
    while !is_last {
        let response = client.receive_message(|_, size| vec![0u8; size]).await;
        if let Ok(msg) = response {
            is_last = match msg {
                DoIpTcpMessage::AliveCheckRequest(_) => true,
                DoIpTcpMessage::AliveCheckResponse(_) => false,
                DoIpTcpMessage::RoutingActivationRequest(_) => true,
                DoIpTcpMessage::RoutingActivationResponse(_) => true,
                DoIpTcpMessage::DiagnosticMessage(req) => {
                    uds_received
                        .lock()
                        .unwrap()
                        .push(req.user_data.get_ref().to_owned());
                    handle_diag_request(&mut client, req).await
                }
                DoIpTcpMessage::DiagnosticMessagePositiveAck(_) => true,
                DoIpTcpMessage::DiagnosticMessageNegativeAck(_) => true,
            };
        } else {
            is_last = true;
        }
    }
}

pub async fn ecu(listener: TcpListener, uds_received: Arc<Mutex<Vec<Vec<u8>>>>) -> io::Result<()> {
    loop {
        match DoIpTcpConnection::accept_doip_tcp(
            &listener,
            0x0e80,
            Timings {
                tcp_connect: Duration::MAX,
                routing_activation_rsp: Duration::from_secs(1),
            },
            |ra| ra.activation_type == ActivationType::Default,
        )
        .await
        {
            Ok(client) => {
                let _ = task::spawn(handle_cnx(client, uds_received.clone())).await;
            }
            Err(_) => {
                error!("Error in connection establishment/routing")
            }
        }
    }
}
