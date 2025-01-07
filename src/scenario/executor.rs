use log::info;
use std::io::Read;
use tokio::time::{self, Duration};

use super::doip_ops::ScenarioMessage;
use super::parser::{self, DisconnectDoIp};
use super::{error::ScenarioError, parser::AbortIfNrc};
use tokio::sync::mpsc;
use uds_rw::{
    message::{self},
    UdsMessage,
};

struct Context {
    last_uds_reply: UdsMessage,
    tx: mpsc::Sender<ScenarioMessage>,
    rx: mpsc::Receiver<ScenarioMessage>,
}

pub async fn execute(
    steps: parser::Steps,
    tx: mpsc::Sender<ScenarioMessage>,
    rx: mpsc::Receiver<ScenarioMessage>,
) -> Result<(), ScenarioError> {
    let mut ctxt: Context = Context {
        last_uds_reply: UdsMessage::RawUds(message::RawUds { data: vec![] }),
        rx,
        tx,
    };
    for step in steps {
        use parser::Step::*;
        match step {
            AbortIfNrc(anrc) => {
                if abort_if_nrc(&ctxt, anrc) {
                    println!("Abort if NRC condition met, aborting scenario.");
                    break;
                }
            }
            DisconnectDoIp(disc) => disconnect_doip(&mut ctxt, disc).await?,
            PrintLastReply => print_last_reply(&ctxt),
            RawUds(ruds) => uds_raw(&mut ctxt, ruds).await?,
            ReadDID(did) => read_did(&mut ctxt, did).await?,
            ReadSupportedDTC(dtc) => read_supported_dtc(&mut ctxt, dtc).await?,
            SleepMs(time_ms) => sleep_ms(&mut ctxt, time_ms).await?,
            WriteDID(did) => write_did(&mut ctxt, did).await?,
            TransferDownload(td) => transfer_download(&mut ctxt, td).await?,
        }
    }
    Ok(())
}

async fn sleep_ms(ctxt: &mut Context, sleep_ms: usize) -> Result<(), ScenarioError> {
    let begin = time::Instant::now();
    loop {
        let mut remain = time::Duration::from_millis(sleep_ms as u64);
        remain = remain.saturating_sub(time::Instant::now() - begin);
        if remain.is_zero() {
            break;
        }
        let sleep = time::sleep(remain);
        tokio::pin!(sleep);
        #[rustfmt::skip]
        tokio::select! {
            _ = &mut sleep => { break },
            rsp = ctxt.rx.recv() => {
		if let Some(ScenarioMessage::AliveCheckReq) = rsp {
		    let _ = ctxt.tx.send(ScenarioMessage::AliveCheckRsp).await;
		}
	    },
        }
    }
    Ok(())
}

async fn disconnect_doip(ctxt: &mut Context, disc: DisconnectDoIp) -> Result<(), ScenarioError> {
    let _ = ctxt.tx.send(ScenarioMessage::DisconnectReconnectReq).await;
    if let Some(wait_after_ms) = disc.wait_after_ms {
        time::sleep(Duration::from_millis(wait_after_ms as u64)).await;
    }
    loop {
        if let Some(ScenarioMessage::NotifyDoIpCnxRoutingAck) = ctxt.rx.recv().await {
            break;
        };
    }
    Ok(())
}

fn abort_if_nrc(ctxt: &Context, anrc: AbortIfNrc) -> bool {
    if let UdsMessage::Nrc(unrc) = &ctxt.last_uds_reply {
        let nrc = unrc.nrc;
        match anrc.nrc {
            Some(nrc_match) => nrc == nrc_match,
            None => true,
        }
    } else {
        false
    }
}

fn print_last_reply(ctxt: &Context) {
    let uds = &ctxt.last_uds_reply;
    println!("Rx UDS: {uds}");
}

fn expect_reply(ctxt: &Context, request_sid: u8) -> Result<(), ScenarioError> {
    let reply = &ctxt.last_uds_reply;
    let reply_sid: u8 = reply.into();
    if request_sid | 0x40 == reply_sid {
        return Ok(());
    }
    match reply {
        UdsMessage::Nrc(nrc) => Err(ScenarioError::Nrc(nrc.nrc)),
        _ => Err(ScenarioError::UnexpectedUdsMessage(reply.clone())),
    }?;
    Ok(())
}

async fn transfer_download(
    ctxt: &mut Context,
    td: parser::TransferDownload,
) -> Result<(), ScenarioError> {
    let mut file = std::fs::File::open(td.filename)?;
    let req = message::RequestDownloadReq {
        compression_method: td.compression_method,
        encryption_method: td.encrypt_method,
        memory_size_bytes: 4,
        memory_address_bytes: 4,
        memory_address: td.addr,
        memory_size: td.memorysize,
    };
    let uds_req = UdsMessage::RequestDownloadReq(req);
    let req_sid: u8 = (&uds_req).into();
    request_response(ctxt, uds_req).await?;
    expect_reply(ctxt, req_sid)?;

    let max_block_size = if let UdsMessage::RequestDownloadRsp(rsp) = &ctxt.last_uds_reply {
        rsp.max_block_size
    } else {
        panic!("Impossible case, please contact the developper");
    };

    let mut req0 = message::TransferDataReq {
        block_sequence_counter: 1,
        // max_block_size is : SID (1 byte) + block_seq_counter (1 byte)
        data: vec![0u8; max_block_size - 1 - 1],
    };
    loop {
        let mut req = req0.clone();
        req0.block_sequence_counter = req0.block_sequence_counter.checked_add(1).unwrap_or(0);
        let nb = file.read(&mut req.data)?;
        req.data.resize(nb, 0);
        let uds_req = UdsMessage::TransferDataReq(req);
        let req_sid: u8 = (&uds_req).into();
        request_response(ctxt, uds_req).await?;
        expect_reply(ctxt, req_sid)?;
        if nb < req0.data.len() {
            break;
        }
    }

    let req = message::TransferExitReq { user_data: vec![] };
    let uds_req = UdsMessage::TransferExitReq(req);
    let req_sid: u8 = (&uds_req).into();
    request_response(ctxt, uds_req).await?;
    expect_reply(ctxt, req_sid)?;

    Ok(())
}

async fn request_response(ctxt: &mut Context, uds: UdsMessage) -> Result<(), ScenarioError> {
    let uds = uds_rw::uds_rawuds_remove_raw(uds);
    info!(target: "uds", "Tx UDS: {uds}");
    let r = ctxt.tx.send(ScenarioMessage::Uds(uds)).await;
    if r.is_err() {
        return Err(ScenarioError::NetworkConnectorDead);
    }
    loop {
        let rsp = ctxt.rx.recv().await;
        if rsp.is_none() {
            return Err(ScenarioError::NetworkConnectorDead);
        }
        let rsp = rsp.unwrap();
        match rsp {
            ScenarioMessage::AliveCheckReq => {
                let _ = ctxt.tx.send(ScenarioMessage::AliveCheckRsp).await;
            }
            ScenarioMessage::Uds(rsp) => {
                info!(target: "uds", "Rx UDS: {rsp}");
                ctxt.last_uds_reply = rsp;
                let reply = &ctxt.last_uds_reply;
                if let UdsMessage::Nrc(rnrc) = reply {
                    if rnrc.nrc == 0x78 {
                        continue;
                    }
                }
                break;
            }
            _ => {
                continue;
            }
        }
    }

    Ok(())
}

async fn uds_raw(ctxt: &mut Context, ruds: parser::RawUds) -> Result<(), ScenarioError> {
    let req = message::RawUds {
        data: ruds.uds_bytes,
    };
    let uds = UdsMessage::RawUds(req);
    request_response(ctxt, uds).await
}

async fn read_did(ctxt: &mut Context, rdid: parser::ReadDID) -> Result<(), ScenarioError> {
    let req = message::ReadDIDReq { did: rdid.did };
    let uds = UdsMessage::ReadDIDReq(req);
    request_response(ctxt, uds).await
}

async fn read_supported_dtc(
    ctxt: &mut Context,
    _rdtc: parser::ReadSupportedDTC,
) -> Result<(), ScenarioError> {
    let req = message::ReadDTCReq {
        sub: message::DTCReqSubfunction::ReportSupportedDTC,
    };
    let uds = UdsMessage::ReadDTCReq(req);
    request_response(ctxt, uds).await
}

async fn write_did(ctxt: &mut Context, wdid: parser::WriteDID) -> Result<(), ScenarioError> {
    let data = match wdid.data {
        parser::FileOrRawBytes::Bytes(bytes) => bytes,
        parser::FileOrRawBytes::BinFileName(bytes) => bytes,
    };
    let req = message::WriteDIDReq {
        did: wdid.did,
        user_data: data,
    };
    let uds = UdsMessage::WriteDIDReq(req);
    request_response(ctxt, uds).await
}
