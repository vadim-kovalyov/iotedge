// Copyright (c) Microsoft. All rights reserved.

#![deny(rust_2018_idioms, warnings)]
#![deny(clippy::all, clippy::pedantic)]
#![allow(clippy::let_unit_value, clippy::similar_names)]

use std::io;
use std::process;

use clap::{crate_description, crate_name, App, AppSettings, Arg, SubCommand};
use failure::{Fail, ResultExt};
use url::Url;

use edgelet_core::{parse_since, LogOptions, LogTail};
use edgelet_http_mgmt::ModuleClient;
use support_bundle::OutputLocation;

use iotedge::{
    Check, Command, Error, ErrorKind, List, Logs, OutputFormat, Restart, SupportBundleCommand,
    Unknown, Version,
};

fn main() {
    if let Err(ref error) = run() {
        let fail: &dyn Fail = error;

        eprintln!("{}", error.to_string());

        for cause in fail.iter_causes() {
            eprintln!("\tcaused by: {}", cause);
        }

        eprintln!();

        process::exit(1);
    }
}

#[allow(clippy::too_many_lines)]
fn run() -> Result<(), Error> {
    let aziot_bin = option_env!("AZIOT_BIN").unwrap_or("aziot");

    let default_mgmt_uri =
        option_env!("IOTEDGE_HOST").unwrap_or("unix:///var/run/iotedge/mgmt.sock");

    let default_diagnostics_image_name = format!(
        "/azureiotedge-diagnostics:{}",
        edgelet_core::version().replace("~", "-")
    );

    let matches = App::new(crate_name!())
        .version(edgelet_core::version_with_source_version())
        .about(crate_description!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("host")
                .help("Daemon socket to connect to")
                .short("H")
                .long("host")
                .takes_value(true)
                .value_name("HOST")
                .global(true)
                .env("IOTEDGE_HOST")
                .default_value(default_mgmt_uri),
        )
        .subcommand(
            SubCommand::with_name("check")
                .about("Check for common config and deployment issues")
                .arg(
                    Arg::with_name("config-file")
                        .short("c")
                        .long("config-file")
                        .value_name("FILE")
                        .help("Sets daemon configuration file")
                        .takes_value(true)
                        .default_value("/etc/aziot/edged/config.yaml"),
                )
                .arg(
                    Arg::with_name("container-engine-config-file")
                        .long("container-engine-config-file")
                        .value_name("FILE")
                        .help("Sets the path of the container engine configuration file")
                        .takes_value(true)
                        .default_value("/etc/docker/daemon.json"),
                )
                .arg(
                    Arg::with_name("diagnostics-image-name")
                        .long("diagnostics-image-name")
                        .value_name("IMAGE_NAME")
                        .help("Sets the name of the azureiotedge-diagnostics image.")
                        .takes_value(true)
                        .default_value(&default_diagnostics_image_name),
                )
                .arg(
                    Arg::with_name("dont-run")
                        .long("dont-run")
                        .value_name("DONT_RUN")
                        .help("Space-separated list of check IDs. The checks listed here will not be run. See 'iotedge check-list' for details of all checks.\n")
                        .multiple(true)
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("expected-aziot-edged-version")
                        .long("expected-aziot-edged-version")
                        .value_name("VERSION")
                        .help("Sets the expected version of the aziot-edged binary. Defaults to the value contained in <http://aka.ms/latest-iotedge-stable>")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("aziot-edged")
                        .long("aziot-edged")
                        .value_name("PATH_TO_AZIOT_EDGED")
                        .help("Sets the path of the aziot-edged binary.")
                        .takes_value(true)
                        .default_value("/usr/libexec/aziot/aziot-edged"),
                )
                .arg(
                    Arg::with_name("iothub-hostname")
                        .long("iothub-hostname")
                        .value_name("IOTHUB_HOSTNAME")
                        .help("Sets the hostname of the Azure IoT Hub that this device would connect to. If using manual provisioning, this does not need to be specified.")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("ntp-server")
                        .long("ntp-server")
                        .value_name("NTP_SERVER")
                        .help("Sets the NTP server to use when checking host local time.")
                        .takes_value(true)
                        .default_value("pool.ntp.org:123"),
                )
                .arg(
                    Arg::with_name("output")
                        .long("output")
                        .short("o")
                        .value_name("FORMAT")
                        .help("Output format. Note that JSON output contains some additional information like OS name, OS version, disk space, etc.")
                        .takes_value(true)
                        .possible_values(&["json", "text"])
                        .default_value("text"),
                )
                .arg(
                    Arg::with_name("verbose")
                        .long("verbose")
                        .value_name("VERBOSE")
                        .help("Increases verbosity of output.")
                        .takes_value(false),
                )
                .arg(
                    Arg::with_name("warnings-as-errors")
                        .long("warnings-as-errors")
                        .value_name("WARNINGS_AS_ERRORS")
                        .help("Treats warnings as errors. Thus 'iotedge check' will exit with non-zero code if it encounters warnings.")
                        .takes_value(false),
                ),
        )
        .subcommand(SubCommand::with_name("check-list").about("List the checks that are run for 'iotedge check'"))
        .subcommand(SubCommand::with_name("list").about("List modules"))
        .subcommand(
            SubCommand::with_name("restart")
                .about("Restart a module")
                .arg(
                    Arg::with_name("MODULE")
                        .help("Sets the module identity to restart")
                        .required(true)
                        .index(1),
                ),
        )
        .subcommand(
            SubCommand::with_name("logs")
                .about("Fetch the logs of a module")
                .arg(
                    Arg::with_name("MODULE")
                        .help("Sets the module identity to get logs")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("tail")
                        .help("Number of lines to show from the end of the log")
                        .long("tail")
                        .takes_value(true)
                        .value_name("NUM")
                        .default_value("all"),
                )
                .arg(
                    Arg::with_name("since")
                        .help("Only return logs since this time, as a duration (1 day, 90 minutes, 2 days 3 hours 2 minutes), rfc3339 timestamp, or UNIX timestamp")
                        .long("since")
                        .takes_value(true)
                        .value_name("DURATION or TIMESTAMP")
                        .default_value("1 day"),
                )
                .arg(
                    Arg::with_name("until")
                        .help("Only return logs up to this time, as a duration (1 day, 90 minutes, 2 days 3 hours 2 minutes), rfc3339 timestamp, or UNIX timestamp. For example, 0d would not truncate any logs, while 2h would return logs up to 2 hours ago")
                        .long("until")
                        .takes_value(true)
                        .value_name("DURATION or TIMESTAMP"),
                )
                .arg(
                    Arg::with_name("follow")
                        .help("Follow output log")
                        .short("f")
                        .long("follow"),
                ),
        )
        .subcommand(
            SubCommand::with_name("init")
                .about("Initialize Azure IoT Edge configuration.")
                .unset_setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("import")
                    .about("Initializing Azure IoT Edge configuration by importing an existing Azure IoT Edge <1.2 configuration.")
                    .arg(
                        Arg::with_name("config-file")
                            .short("c")
                            .long("config-file")
                            .value_name("FILE")
                            .help("Sets path of old IoT Edge configuration file")
                            .takes_value(true)
                            .default_value("/etc/iotedge/config.yaml"),
                    )
                )
        )
        .subcommand(
            SubCommand::with_name("support-bundle")
                .about("Bundles troubleshooting information")
                .arg(
                    Arg::with_name("output")
                        .help("Location to output file. Use - for stdout")
                        .long("output")
                        .short("o")
                        .takes_value(true)
                        .value_name("FILENAME")
                        .default_value("support_bundle.zip"),
                )
                .arg(
                    Arg::with_name("since")
                        .help("Only return logs since this time, as a duration (1d, 90m, 2h30m), rfc3339 timestamp, or UNIX timestamp")
                        .long("since")
                        .takes_value(true)
                        .value_name("DURATION or TIMESTAMP")
                        .default_value("1 day"),
                )
                .arg(
                    Arg::with_name("until")
                        .help("Only return logs up to this time, as a duration (1 day, 90 minutes, 2 days 3 hours 2 minutes), rfc3339 timestamp, or UNIX timestamp. For example, 0d would not truncate any logs, while 2h would return logs up to 2 hours ago")
                        .long("until")
                        .takes_value(true)
                        .value_name("DURATION or TIMESTAMP")
                )
                .arg(
                    Arg::with_name("include-edge-runtime-only")
                        .help("Only include logs from Microsoft-owned Edge modules")
                        .long("include-edge-runtime-only")
                        .short("e")
                        .takes_value(false),
                ).arg(
                    Arg::with_name("iothub-hostname")
                        .long("iothub-hostname")
                        .value_name("IOTHUB_HOSTNAME")
                        .help("Sets the hostname of the Azure IoT Hub that this device would connect to. If using manual provisioning, this does not need to be specified.")
                        .takes_value(true),
                ).arg(
                    Arg::with_name("quiet")
                        .help("Suppress status output")
                        .long("quiet")
                        .short("q")
                        .takes_value(false),
                ),
        )
        .subcommand(SubCommand::with_name("version").about("Show the version information"))
        .get_matches();

    let runtime = || -> Result<_, Error> {
        let url = matches.value_of("host").map_or_else(
            || Err(Error::from(ErrorKind::MissingHostParameter)),
            |h| {
                Url::parse(h)
                    .context(ErrorKind::BadHostParameter)
                    .map_err(Error::from)
            },
        )?;
        let runtime = ModuleClient::new(&url).context(ErrorKind::ModuleRuntime)?;
        Ok(runtime)
    };

    let mut tokio_runtime = tokio::runtime::Runtime::new().context(ErrorKind::InitializeTokio)?;

    match matches.subcommand() {
        ("check", Some(args)) => {
            let check = Check::new(
                args.value_of_os("config-file")
                    .expect("arg has a default value")
                    .to_os_string()
                    .into(),
                args.value_of_os("container-engine-config-file")
                    .expect("arg has a default value")
                    .to_os_string()
                    .into(),
                args.value_of("diagnostics-image-name")
                    .expect("arg has a default value")
                    .to_string(),
                args.values_of("dont-run")
                    .into_iter()
                    .flatten()
                    .map(ToOwned::to_owned)
                    .collect(),
                args.value_of("expected-aziot-edged-version")
                    .map(ToOwned::to_owned),
                args.value_of_os("aziot-edged")
                    .expect("arg has a default value")
                    .to_os_string()
                    .into(),
                args.value_of("output")
                    .map(|arg| match arg {
                        "json" => OutputFormat::Json,
                        "text" => OutputFormat::Text,
                        _ => unreachable!(),
                    })
                    .expect("arg has a default value"),
                args.is_present("verbose"),
                args.is_present("warnings-as-errors"),
                aziot_bin.into(),
                args.value_of("iothub-hostname").map(ToOwned::to_owned),
            );

            tokio_runtime.block_on(check)?.execute(&mut tokio_runtime)
        }
        ("check-list", Some(_)) => Check::print_list(aziot_bin.into()),
        ("list", _) => tokio_runtime.block_on(List::new(runtime()?, io::stdout()).execute()),
        ("restart", Some(args)) => tokio_runtime.block_on(
            Restart::new(
                args.value_of("MODULE").unwrap().to_string(),
                runtime()?,
                io::stdout(),
            )
            .execute(),
        ),
        ("logs", Some(args)) => {
            let id = args.value_of("MODULE").unwrap().to_string();
            let follow = args.is_present("follow");
            let tail = args
                .value_of("tail")
                .map(str::parse)
                .transpose()
                .map_err(|err: edgelet_core::Error| {
                    Error::from(err.context(ErrorKind::BadTailParameter))
                })?
                .expect("arg has a default value");
            let since = args
                .value_of("since")
                .map(|s| parse_since(s))
                .transpose()
                .context(ErrorKind::BadSinceParameter)?
                .expect("arg has a default value");
            let mut options = LogOptions::new()
                .with_follow(follow)
                .with_tail(tail)
                .with_since(since);
            if let Some(until) = args
                .value_of("until")
                .map(|s| parse_since(s))
                .transpose()
                .context(ErrorKind::BadSinceParameter)?
            {
                options = options.with_until(until);
            }
            tokio_runtime.block_on(Logs::new(id, options, runtime()?).execute())
        }
        ("init", Some(args)) => match args.subcommand() {
            ("", _) => {
                let () = iotedge::init::execute().map_err(ErrorKind::Init)?;
                Ok(())
            }
            ("import", Some(args)) => {
                let old_config_file = args
                    .value_of_os("config-file")
                    .expect("arg has a default value");
                let old_config_file = std::path::Path::new(old_config_file);
                let () =
                    iotedge::init::import::execute(old_config_file).map_err(ErrorKind::Init)?;
                Ok(())
            }
            (command, _) => {
                eprintln!("Unknown init subcommand {:?}", command);
                std::process::exit(1);
            }
        },
        ("support-bundle", Some(args)) => {
            let location = args.value_of_os("output").expect("arg has a default value");
            let since = args
                .value_of("since")
                .map(|s| parse_since(s))
                .transpose()
                .context(ErrorKind::BadSinceParameter)?
                .expect("arg has a default value");
            let mut options = LogOptions::new()
                .with_follow(false)
                .with_tail(LogTail::All)
                .with_since(since);
            if let Some(until) = args
                .value_of("until")
                .map(|s| parse_since(s))
                .transpose()
                .context(ErrorKind::BadSinceParameter)?
            {
                options = options.with_until(until);
            }
            let include_ms_only = args.is_present("include-edge-runtime-only");
            let verbose = !args.is_present("quiet");
            let iothub_hostname = args.value_of("iothub-hostname").map(ToOwned::to_owned);
            let output_location = if location == "-" {
                OutputLocation::Memory
            } else {
                OutputLocation::File(location.to_owned())
            };

            tokio_runtime.block_on(
                SupportBundleCommand::new(
                    options,
                    include_ms_only,
                    verbose,
                    iothub_hostname,
                    output_location,
                    runtime()?,
                )
                .execute(),
            )
        }
        ("version", _) => tokio_runtime.block_on(Version::new().execute()),
        (command, _) => tokio_runtime.block_on(Unknown::new(command.to_string()).execute()),
    }
}
