use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

use super::ecu;
use crate::scenario::{self, parser::Steps};

pub async fn run_test_scenario_str(s: &str) -> Result<Vec<Vec<u8>>, String> {
    let steps = scenario::parser::read_scenario_str(s);
    test_scenario(steps).await
}

pub async fn run_test_scenario_file(filename: &str) -> Result<Vec<Vec<u8>>, String> {
    let steps = scenario::parser::read_scenario(filename);
    test_scenario(steps).await
}

async fn test_scenario(steps: Steps) -> Result<Vec<Vec<u8>>, String> {
    let _ = env_logger::try_init();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| format!("tcp listener creation failed: {err:?}"))?;

    let local_addr = "0.0.0.0:0".parse().unwrap();
    let remote_addr = listener.local_addr().unwrap();
    let doip_la = 0x00ed;
    let doip_ta = 0x0077;

    let received = Arc::new(Mutex::new(Vec::new()));
    tokio::select!(
            res = async {
        ecu::ecu(listener, received.clone())
                    .await
                    .map_err(|err| format!("ecu simulator finished on error: {err:?}"))
            } => res,
            res = async {
        scenario::main::scenario(local_addr, remote_addr, doip_la, doip_ta, steps)
                    .await
                    .map_err(|err| format!("scenario finished on error: {err:?}"))
            } => res,
    )
    .map(|_| received.lock().unwrap().to_owned())
}

pub fn uds_seq(strs: &[&str]) -> Vec<Vec<u8>> {
    strs.iter()
        .map(|s| {
            s.split_ascii_whitespace()
                .map(|s| u8::from_str_radix(s, 16).unwrap())
                .collect::<Vec<u8>>()
        })
        .collect()
}
