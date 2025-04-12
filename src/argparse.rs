use bpaf::Bpaf;
use serde::Deserialize;
use std::net::SocketAddr;

/// Argument of the program, resulting from an aggregation of default values,
/// optional command line options, and optional configuration file.
pub struct Args {
    /// Local UDP and TCP socket address for binding local socket for our diagtool
    pub local_addr: SocketAddr,
    /// Remote UDP and TCP socket address for sending packets to a remote
    /// DoIP server/provider
    pub remote_addr: SocketAddr,
    /// Broadcast UDP socket address for Vehicle Announcement breadcast.
    pub broadcast_addr: SocketAddr,
    /// Discovery enabling
    pub discover: bool,
    /// DoIP local address for diagtool
    pub doip_la: u16,
    /// DoIP remote address of the targeted diag provider
    pub doip_ta: u16,
    /// List of UDS commands to send
    pub uds_commands: Option<Vec<Vec<u8>>>,
    /// Scenario to execute
    pub scenario: Option<String>,
}

/// Parse commandline
///
/// Parse commandline arguments, augment with a potential commandline provided
/// configuration file, and return the result.
pub fn get_args() -> Args {
    _get_args()
}

fn parse_hex_u8(ins: &str) -> Option<u8> {
    u8::from_str_radix(ins, 16).ok()
}

fn parse_hex_u8_multiple(ins: &str) -> Option<(u8, usize)> {
    let ss: Vec<&str> = ins.split('*').collect();
    let cardinality: Option<usize> = match ss.len() {
        1 => Some(1),
        2 => ss[1].parse().ok(),
        _ => None,
    };
    let value = parse_hex_u8(ss[0]);
    if let (Some(cardinality), Some(value)) = (cardinality, value) {
        Some((value, cardinality))
    } else {
        None
    }
}

fn parse_u16(ins: &str) -> Option<u16> {
    ins.parse::<u16>()
        .or_else(|_| u16::from_str_radix(ins, 16))
        .or_else(|_| u16::from_str_radix(&ins[2..], 16))
        .ok()
}

fn parse_uds_command(ins: &str) -> Option<Vec<u8>> {
    let atoms = ins.split(' ').map(parse_hex_u8_multiple);
    let atoms = atoms.into_iter().collect::<Option<Vec<(u8, usize)>>>();
    match atoms {
        Some(atoms) => {
            let mut v = Vec::new();
            for atom in atoms.into_iter() {
                v.extend_from_slice(&vec![atom.0; atom.1]);
            }
            Some(v)
        }
        None => None,
    }
}

fn parse_uds_commands(ins: Vec<&str>) -> Option<Vec<Vec<u8>>> {
    ins.into_iter()
        .map(parse_uds_command)
        .collect::<Option<Vec<Vec<u8>>>>()
}

#[derive(Debug, Clone, Bpaf, Deserialize, Default)]
#[bpaf(options)]
struct Options {
    #[bpaf(long)]
    /// Local DoIP socket address to bind in TCP and UDP, format IPv4:port, such as "192.168.11.10:0" for DDT2000 equivalent.
    /// Default value is 192.168.11.10:0
    local_diag_socket: Option<String>,
    #[bpaf(long)]
    /// Remote DoIP socket address to bind in TCP and UDP, format IPv4:port, such as "192.168.11.51:0" for PCU AP equivalent
    /// Default value is 192.168.11.53:13400
    remote_diag_socket: Option<String>,
    #[bpaf(long)]
    /// DoIP broadcast socket address to bind in and UDP, for VIN discovery, format IPv4:port, such as "255.255.255.255:13400"
    /// Default value is 255.255.255.255:13400
    broadcast_diag_socket: Option<String>,
    #[bpaf(
        long,
        guard(|x| x.is_none() || parse_u16(x.clone().unwrap().as_str()).is_some(), "`doip_local_addr must be of 0xXYUV form, like 0x00ed`")
    )]
    /// Discovery enabling (VIR)
    #[bpaf(flag(true, false))]
    discover: bool,
    /// DoIP local address, format 0xUVXY, such as "0xe080" for DDT2000 equivalent.
    /// Default value is 0xe080
    doip_local_addr: Option<String>,
    #[bpaf(
        long,
        guard(|x| x.is_none() || parse_u16(x.clone().unwrap().as_str()).is_some(), "`doip_target_addr must be of 0xXYUV form, like 0x00ed`")
    )]
    /// DoIP target address, format 0xUVXY, such as "0x000ed" for PCU AP
    /// Default value is 0x00ed
    doip_target_addr: Option<String>,
    #[bpaf(long)]
    /// Optional yaml config file to preconfigure all command line arguments.
    /// Fields are named the same, ie. broadcast_diag_socket, ... The field for
    /// UDS commands is uds_commands, an array of strings.
    /// UDS commands in the config are run before the ones on command line.
    configfile: Option<String>,
    #[bpaf(long)]
    /// Optional yaml config file to launch a scenario.
    /// A scenario is a list of uds command and special shortcuts, like TransferDownload
    /// Can be several scenarii separated by a comma, such as "--scenario dtc0a.yaml,reprog_fd01.yaml"
    scenario: Option<String>,
    /// UDS commands to launch, such as "10 03" "22 02" or "22 02 FF*12"
    #[bpaf(positional("UDS commands"), guard(|x| parse_uds_commands(x.iter().map(|s| &**s).collect()).is_some(), "commands should be space separated quoted strings of space separated double-hexa-nibbles"))]
    uds_commands: Vec<String>,
}

fn override_opts(src: Options, overrider: Options) -> Options {
    let uds_commands = [src.uds_commands, overrider.uds_commands].concat();
    Options {
        local_diag_socket: overrider.local_diag_socket.or(src.local_diag_socket),
        remote_diag_socket: overrider.remote_diag_socket.or(src.remote_diag_socket),
        broadcast_diag_socket: overrider
            .broadcast_diag_socket
            .or(src.broadcast_diag_socket),
        discover: overrider.discover || src.discover,
        doip_local_addr: overrider.doip_local_addr.or(src.doip_local_addr),
        doip_target_addr: overrider.doip_target_addr.or(src.doip_target_addr),
        configfile: overrider.configfile.or(src.configfile),
        uds_commands,
        scenario: overrider.scenario.or(src.scenario),
    }
}

pub fn _get_args() -> Args {
    let default_opts = Options {
        local_diag_socket: Some("192.168.11.10:0".to_string()),
        remote_diag_socket: Some("192.168.11.53:13400".to_string()),
        broadcast_diag_socket: Some("255.255.255.255:13400".to_string()),
        discover: false,
        doip_local_addr: Some("0xe080".to_string()),
        doip_target_addr: Some("0x00ed".to_string()),
        configfile: None,
        uds_commands: vec![],
        scenario: None,
    };
    let commandline_opts = options().run();
    let filename_opts = match &commandline_opts.configfile {
        Some(filename) => configfile::read_file(filename),
        None => Options::default(),
    };
    let opts = override_opts(default_opts, filename_opts);
    let opts = override_opts(opts, commandline_opts);

    let local_addr = opts.local_diag_socket.unwrap().parse().unwrap();
    let remote_addr = opts.remote_diag_socket.unwrap().parse().unwrap();
    let broadcast_addr = opts.broadcast_diag_socket.unwrap().parse().unwrap();
    let discover = opts.discover;
    let doip_la = parse_u16(&opts.doip_local_addr.unwrap_or("0x0e80".to_string())).unwrap();
    let doip_ta = parse_u16(&opts.doip_target_addr.unwrap_or("0x00ed".to_string())).unwrap();
    let uds_commands = parse_uds_commands(opts.uds_commands.iter().map(|s| &**s).collect());
    let scenario = opts.scenario;
    Args {
        local_addr,
        remote_addr,
        broadcast_addr,
        discover,
        doip_la,
        doip_ta,
        uds_commands,
        scenario,
    }
}

mod configfile {
    use super::Options;
    use serde_yaml::from_reader;

    pub(super) fn read_file(filename: &str) -> Options {
        let f = std::fs::File::open(filename)
            .inspect_err(|err| println!("Can't open configuration file {filename}: {err}"))
            .unwrap();
        let opts: Options = from_reader(f).unwrap();
        opts
    }
}
