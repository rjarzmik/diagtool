use evalexpr::{
    context_map, ContextWithMutableVariables, DefaultNumericTypes, EvalexprError, HashMapContext,
    Value,
};
use log::{debug, info};
use pretty_hex::pretty_hex;
use std::future::Future;
use std::io::{self, Read};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::time::{self, Duration};
use uds_rw::uds_write;

use super::doip_ops::ScenarioMessage;
use super::parser::{self, DisconnectDoIp, Step};
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
    eval_expr: EvalExprContext,
}

fn execute_step<'b: 'a, 'a>(
    ctxt: &'a mut Context,
    step: &'b Step,
) -> Pin<Box<dyn Future<Output = Result<bool, ScenarioError>> + 'a>> {
    Box::pin(async move {
        let mut abort = false;
        use parser::Step::*;

        debug!("Executing step: {step:?}");
        match step {
            AbortIfNrc(anrc) => {
                if abort_if_nrc(ctxt, anrc) {
                    println!("Abort if NRC condition met, aborting scenario.");
                    abort = true;
                }
            }
            DisconnectDoIp(disc) => disconnect_doip(ctxt, disc).await?,
            EvalExpr(expr) => eval_expr(ctxt, expr)?,
            PrintLastReply => print_last_reply(ctxt),
            RawUds(ruds) => uds_raw(ctxt, ruds).await?,
            ReadDID(did) => read_did(ctxt, did).await?,
            ReadSupportedDTC(dtc) => read_supported_dtc(ctxt, dtc).await?,
            SleepMs(time_ms) => sleep_ms(ctxt, *time_ms).await?,
            WhileLoop(wl) => {
                if while_loop(ctxt, wl).await? {
                    println!("While loop aborted scenario.");
                    abort = true;
                }
            }
            WriteDID(did) => write_did(ctxt, did).await?,
            TransferDownload(td) => transfer_download(ctxt, td).await?,
        };
        Ok(abort)
    })
}

async fn execute_steps(ctxt: &mut Context, steps: &Vec<Step>) -> Result<bool, ScenarioError> {
    let mut abort = false;
    for step in steps {
        abort = execute_step(ctxt, step).await?;
        if abort {
            break;
        }
    }
    Ok(abort)
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
        eval_expr: EvalExprContext::new(),
    };

    execute_steps(&mut ctxt, &steps).await.map(|_| ())
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

async fn disconnect_doip(ctxt: &mut Context, disc: &DisconnectDoIp) -> Result<(), ScenarioError> {
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

fn abort_if_nrc(ctxt: &Context, anrc: &AbortIfNrc) -> bool {
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
    td: &parser::TransferDownload,
) -> Result<(), ScenarioError> {
    let mut file = std::fs::File::open(&td.filename)?;
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
                ctxt.eval_expr.set_reply(&rsp);
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

async fn uds_raw(ctxt: &mut Context, ruds: &parser::RawUds) -> Result<(), ScenarioError> {
    let req = message::RawUds {
        data: ruds
            .data
            .get_bytes(|varname| ctxt.eval_expr.get_tuple_variable(varname))?,
    };
    let uds = UdsMessage::RawUds(req);
    request_response(ctxt, uds).await
}

async fn read_did(ctxt: &mut Context, rdid: &parser::ReadDID) -> Result<(), ScenarioError> {
    let req = message::ReadDIDReq { did: rdid.did };
    let uds = UdsMessage::ReadDIDReq(req);
    request_response(ctxt, uds).await
}

async fn read_supported_dtc(
    ctxt: &mut Context,
    _rdtc: &parser::ReadSupportedDTC,
) -> Result<(), ScenarioError> {
    let req = message::ReadDTCReq {
        sub: message::DTCReqSubfunction::ReportSupportedDTC,
    };
    let uds = UdsMessage::ReadDTCReq(req);
    request_response(ctxt, uds).await
}

async fn write_did(ctxt: &mut Context, wdid: &parser::WriteDID) -> Result<(), ScenarioError> {
    let user_data = wdid
        .data
        .get_bytes(|varname| ctxt.eval_expr.get_tuple_variable(varname))?;
    let req = message::WriteDIDReq {
        did: wdid.did,
        user_data,
    };
    let uds = UdsMessage::WriteDIDReq(req);
    request_response(ctxt, uds).await
}

fn eval_expr(ctxt: &mut Context, expr: &parser::EvalExpr) -> Result<(), ScenarioError> {
    expr.expression
        .compiled
        .eval_empty_with_context_mut(&mut ctxt.eval_expr.ctxt)
        .map_err(|err| ScenarioError::EvalExpr(expr.expression.str.clone(), err))?;
    Ok(())
}

async fn while_loop(ctxt: &mut Context, wl: &parser::WhileLoop) -> Result<bool, ScenarioError> {
    let mut abort = false;
    while !abort {
        let cond = wl
            .condition
            .compiled
            .eval_boolean_with_context_mut(&mut ctxt.eval_expr.ctxt)
            .map_err(|err| ScenarioError::EvalExpr(wl.condition.str.clone(), err))?;
        if cond {
            abort = execute_steps(ctxt, &wl.steps).await?;
        } else {
            break;
        }
    }
    Ok(abort)
}

struct EvalExprContext {
    ctxt: HashMapContext<DefaultNumericTypes>,
    reply: Arc<Mutex<Vec<u8>>>,
}

impl EvalExprContext {
    pub fn new() -> Self {
        let reply = Arc::new(Mutex::new(Vec::<u8>::new()));
        let reply_c1 = reply.clone();
        let ctxt: HashMapContext<DefaultNumericTypes> = context_map! {
            "reply_nth" => Function::new(move |argument| {
                if let Ok(int) = argument.as_int() {
                    reply_c1.lock().unwrap().get(int as usize)
			.map(|v| Value::Int((*v) as i64))
			.ok_or(EvalexprError::OutOfBoundsAccess)
                } else {
                    Err(EvalexprError::expected_int(argument.clone()))
                }
            }),
            "print" => Function::new(move |argument| {
                let s : String = match argument {
                    Value::String(s) => s.to_owned(),
                    Value::Float(f) => (f as &f64).to_string(),
                    Value::Int(i) => (i as &i64).to_string(),
                    Value::Boolean(b) => b.to_string(),
                    Value::Tuple(vec) => {
			let v = Self::value_to_bytes(vec);
			pretty_hex(&v)},
                    Value::Empty => "".to_string(),
                };
                println!("{s}");
                Ok(Value::Empty)}),
	    "loadfile" => Function::new(move |argument| {
                if let Ok(s) = &argument.as_string() {
		    if let Some(v) = Self::load_file(s) {
			Ok(Value::Tuple(v.iter().map(|b| Value::<DefaultNumericTypes>::from_int(*b as i64)).collect()))
		    } else {
			Err(EvalexprError::CustomMessage(format!("Can't load file {s}")))
		    }
		} else {
                    Err(EvalexprError::expected_string(argument.clone()))
		}
	    }),
        }
        .unwrap();
        Self { ctxt, reply }
    }

    pub fn set_reply(&mut self, uds_reply: &UdsMessage) {
        let mut reply: Vec<u8> = vec![];
        uds_write(&mut reply, uds_reply).unwrap();
        let _ = self.ctxt.set_value(
            "reply".to_string(),
            Value::Tuple(reply.iter().map(|r| Value::Int(*r as i64)).collect()),
        );
        *self.reply.lock().unwrap() = reply;
    }

    pub fn get_tuple_variable(&self, varname: &str) -> Result<Vec<u8>, io::Error> {
        use evalexpr::Context;
        self.ctxt
            .get_value(varname)
            .and_then(|varvalue| match varvalue {
                Value::Tuple(vec) => Some(Self::value_to_bytes(vec)),
                Value::String(s) => Some(s.as_bytes().to_vec()),
                _ => None,
            })
            .ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Evalexpr variable {varname} doesn't exist."),
            ))
    }

    fn load_file(filename: &str) -> Option<Vec<u8>> {
        std::fs::read(filename).ok()
    }

    fn flat_vec(input: &[Value]) -> Vec<Value> {
        input
            .iter()
            .flat_map(|v| match v {
                Value::String(ref s) => {
                    s.as_bytes().iter().map(|b| Value::Int(*b as i64)).collect()
                }
                Value::Tuple(tuple) => Self::flat_vec(tuple),
                o => vec![o.clone()],
            })
            .collect()
    }

    fn value_to_bytes(input: &[Value]) -> Vec<u8> {
        Self::flat_vec(input)
            .iter()
            .map(|v| if let Value::Int(i) = v { *i as u8 } else { 0 })
            .collect::<Vec<u8>>()
    }
}
