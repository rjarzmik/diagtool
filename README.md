# Diagtool

Diagtool is a simple tool to send UDS commands to a remote ECU.
It communicates over DoIP/TCP/IPv4.
Its main purpose is to have a simple tool to query a car's computer to retrieve information such as a list of failures, its VIN, etc ...

The tool offers also a scenario capability, where a list of commands can be
prepared in a yaml file (ie. the scenario), and the tool will issue them one by
one.

## Information to gather before launching the tool

As for every UDS/DoIP communication, one must know before hand :

 - the logical DoIP address of the remote ECU to diagnostic, such as `0x0080`.
 - the logical DoIP address of diagtool, such as `0xa100`.
 - the IPv4 address and TCP port of the remote ECU to diagnostic (or of a DoIP
   Gateway leading to it). This must be "TCP reachable" from the host diagtool is
   run from, such as `192.168.4.2:13400`.
 - the IPv4 address of diagtool and its local port
   This must be an address of one of the network interfaces diagtool is run
   from. The TCP port might be 0 to create a dynamic number, such as `192.168.4.1:0`.

## Command line only usage
To get all possible invocation, run :
```bash
diagtool --help
```

### Example: Read the VIN
```bash
diagtool --local-diag-socket=192.168.4.1:0 --remote-diag-socket=192.168.4.2:13400 --doip-local-addr=0xa100 --doip-target-addr=0x0080 "22 f1 90"
```

### Example: Write the VIN
```bash
diagtool --local-diag-socket=192.168.4.1:0 --remote-diag-socket=192.168.4.2:13400 --doip-local-addr=0xa100 --doip-target-addr=0x0080 "2e f1 90 12 22 44 11"
```

## Command line with a config file usage
With a configuration containing all the information to launch the tool, the above read VIN can be shortened to :

```bash
diagtool --configfile config.yaml "22 f1 90"
```

where `config.yaml` contains :

```yaml
local_diag_socket: 192.168.4.1:0
remote_diag_socket: 192.168.4.2:13400
broadcast_diag_socket: 255.255.255.255:13400
discover: false
doip_local_addr: 0xa100
doip_target_addr: 0x0080
uds_commands:
```

A couple of examples of configuration files is [here](./config/).

The configuration should alleviate the need to retype DoIP connection information
for each command.

## Command line with a scenario
This is the most complicated usage, where the user wants to perform several UDS
requests/responses, add sleep calls, add conditional execution, use pretty
printing of UDS responses, abort the UDS commands if one of them returns an NRC,
etc ...

### Example to read the VIN, wait 1s, and read DTC
```bash
diagtool --configfile config.yaml --scenario read_vin_read_dtc.yaml
```

where `read_vin_read_dtc.yaml` is :

```yaml
- !ReadDID
  did: 0x0f190
- !PrintLastReply
- !SleepMs 1000
- !RawUds
  uds_bytes: 19 0a
- !PrintLastReply
```

### Reference and possibilities
All the keywords for the scenarii can be found in
[here](./scenario/reference/references_all.yaml).

The detailed keywords with their description and all usages can all be found
under the [directory](./scenario/reference).

For more specific examples, or inspiration can be found in
[here](./scenario/examples).

### Evalexpr expressions
The `EvalExpr` and `WhileLoop` keywords use
[evalexpr](https://docs.rs/evalexpr/latest/evalexpr). There are some hints that
might help to speed you up :
 - if a variable is assigned in one step, it will be available for all steps
 - if a variable is assigned in one step, it can be reassigned in another step, but only with the same type :
   - `a = 3; a = 4` is valid
   - `a = 3: a = "VF1R";` is invalid
 - the `if` condition is written as output = `if(condition, value_if_true, value_if_false)`

If you want to see how to use `EvalExpr`, see
[here](./scenario/examples/write_vin.yaml) the many different ways of writing a
VIN, demonstrating :
 - computing a VIN / DID
 - synchronization, aka. waiting in a scenario for an external event (through
   file reading)
 - etc ...

### Debugging a scenario ###
Don't forget useful logs, such as steps debugging :
```bash
RUST_LOG=diagtool=debug diagtool --configfile config/local_integration_test.yaml --scenario ./scenario/examples/write_vin.yaml
```

Or network oriented debugging :
```bash
RUST_LOG=uds=debug,doip=debug diagtool --configfile config/local_integration_test.yaml --scenario ./scenario/examples/write_vin.yaml
```
