use super::utils;
use tokio::sync::Notify;
use std::sync::Arc;
use anyhow::{Context, Result};
use std::str;
use regex::Regex;

const PROXY_CONFIG_TAG:&str = "proxy config"; 
const PROXY_CONFIG_PATH_RAW:&str = "/app/nginx_default_config.conf";
const PROXY_CONFIG_PATH_PARSED:&str = "/app/nginx_config.conf";
const PROXY_CONFIG_DEFAULT_VARS_LIST:&str = "NGINX_DEFAULT_PORT,NGINX_HAS_BLOB_MODULE,NGING_BLOB_MODULE_NAME_ADDRESS,NGINX_HAS_REGISTRY_MODULE,NGINX_HAS_REGISTRY_MODULE,NGING_REGISTRY_MODULE_ADDRESS,NGINX_NOT_ROOT,GATEWAY_HOSTNAME";
const TWIN_PROXY_CONFIG_KEY:&str = "nginx_config";

const PROXY_CONFIG_DEFAULT_VALUES:&'static [(&str, &str)] = &[("NGINX_DEFAULT_PORT","443")];


fn duration_from_secs_str(s: &str) -> Result<std::time::Duration, <u64 as std::str::FromStr>::Err> {
	Ok(std::time::Duration::from_secs(s.parse()?))
}


#[derive(Debug, structopt::StructOpt)]
struct Options {
	#[structopt(help = "Whether to use websockets or bare TLS to connect to the Iot Hub", long = "use-websocket")]
	use_websocket: bool,

	#[structopt(help = "Will message to publish if this client disconnects unexpectedly", long = "will")]
	will: Option<String>,

	#[structopt(
		help = "Maximum back-off time between reconnections to the server, in seconds.",
		long = "max-back-off",
		default_value = "30",
		parse(try_from_str = duration_from_secs_str),
	)]
	max_back_off: std::time::Duration,

	#[structopt(
		help = "Keep-alive time advertised to the server, in seconds.",
		long = "keep-alive",
		default_value = "5",
		parse(try_from_str = duration_from_secs_str),
	)]
	keep_alive: std::time::Duration,

	#[structopt(
		help = "Interval at which the client reports its twin state to the server, in seconds.",
		long = "report-twin-state-period",
		default_value = "5",
		parse(try_from_str = duration_from_secs_str),
	)]
	report_twin_state_period: std::time::Duration,
}

pub fn start(runtime_handle: tokio::runtime::Handle, notify_received_config: Arc<Notify>){
	use futures_util::StreamExt;

	let Options {
		use_websocket,
		will,
		max_back_off,
		keep_alive,
		report_twin_state_period,
	} = structopt::StructOpt::from_args();

	//Set default value for some environment variables here
	set_default_env_vars();

	let mut client = azure_iot_mqtt::module::Client::new_for_edge_module(
		if use_websocket { azure_iot_mqtt::Transport::WebSocket } else { azure_iot_mqtt::Transport::Tcp },

		will.map(Into::into),

		max_back_off,
		keep_alive,
	).expect("could not create client");

	spawn_background_tasks(
		&runtime_handle,
		client.inner().shutdown_handle(),
		client.report_twin_state_handle(),
		report_twin_state_period,
	);

	//Parse default config and notify to reboot nginx if it has already started
	//If the config is incorrect, panic because otherwise nginx doesn't have any config.
	parse_config().expect("Unable to read default configuration");
    match parse_config() {
		//Notify watchdog config is there
        Ok(()) => notify_received_config.notify(),
        Err(error) => panic!("Error while parsing default config: {:?}", error),
	};

	while let Some(message) = runtime_handle.block_on(client.next()) {
		let message = message.unwrap();

		log::info!("received message {:?}", message);

		if let azure_iot_mqtt::module::Message::TwinPatch(twin) = message {
			if let Err(err) = save_raw_config(&twin)
			{
				log::error!("received message {:?}", err);
			}else
			{
				//Here we don't need to panic if config is wrong. There is already a good config running.
				match parse_config() {
					//Notify watchdog config is there
					Ok(()) => notify_received_config.notify(),
					Err(error) => log::error!("Error while parsing default config: {:?}", error),
				};		
			}
		};
	}
}

fn set_default_env_vars() -> () {
	
	for (key, value) in PROXY_CONFIG_DEFAULT_VALUES.iter()
	{
		match std::env::var(key){
			//If env variable is already declared, do nothing
			Ok(_) => continue,
			//Else add the default value
			Err(_) =>std::env::set_var(key, value)		
		};	
	}	
}

fn save_raw_config(twin: &azure_iot_mqtt::TwinProperties)  -> Result<()>
{
	let json = twin.properties.get_key_value(TWIN_PROXY_CONFIG_KEY);

	//Get value associated with the key and extract is as a string.
	let str = (*(json.context(format!("Key {} not found in twin", PROXY_CONFIG_TAG))?.1)).
			as_str().context("Cannot extract json as base64 string")?;

	let bytes = get_raw_config(str)?;

	utils::write_binary_to_file(&bytes,PROXY_CONFIG_PATH_RAW)?;

	Ok(())
}

fn parse_config()  -> Result<()>
{
	//Read "raw configuration". Contains environment variables and sections.
	//Extract IO calls from core function for mocking
	let str = utils::get_string_from_file(PROXY_CONFIG_PATH_RAW)?;

	let str = get_parsed_config(&str)?;
	//Extract IO calls from core function for mocking
	utils::write_binary_to_file(&str.as_bytes(),PROXY_CONFIG_PATH_PARSED)?;

	Ok(())
}

fn get_raw_config(encoded_file: &str)  -> Result<Vec<u8>, anyhow::Error>
{
	let bytes = match base64::decode(encoded_file)
	{
		Ok(bytes) => bytes,
		Err(err) => return Err(anyhow::anyhow!(format!("Cannot decode base64 {:?}", err))),
	};

	Ok(bytes)
}


fn get_parsed_config(str: &str) -> Result<String, anyhow::Error>
{
	let mut context = std::collections::HashMap::new();

	//Check if user passed their own env variable list.
	let vars = match std::env::var("NGINX_CONFIG_ENV_VAR_LIST"){
		Ok(vars) => vars,
		//@TO CHECK It copies the string, is that ok?
		Err(_) => PROXY_CONFIG_DEFAULT_VARS_LIST.to_string(), 
	};
	let vars = vars.split(',');

	for key in vars{
		let val = match std::env::var(key){
			Ok(val) => val,
			Err(_) => "0".to_string()		
		};
		context.insert(key.to_string(), val);
	}

	let str: String = envsubst::substitute(str, &context).unwrap();
	let re = Regex::new(r"#if_tag 0((.|\n)*?)#endif_tag 0").unwrap();
	let str2 = re.replace_all(&str, "").to_string();

	Ok(str2)
}

fn spawn_background_tasks(
	runtime_handle: &tokio::runtime::Handle,
	shutdown_handle: Result<mqtt3::ShutdownHandle, mqtt3::ShutdownError>,
	mut report_twin_state_handle: azure_iot_mqtt::ReportTwinStateHandle,
	report_twin_state_period: std::time::Duration,
) {
	let mut shutdown_handle = shutdown_handle.expect("couldn't get shutdown handle");
	runtime_handle.spawn(async move {
		let () = tokio::signal::ctrl_c().await.expect("couldn't get Ctrl-C notification");
		let result = shutdown_handle.shutdown().await;
		let () = result.expect("couldn't send shutdown notification");
	});

	runtime_handle.spawn(async move {
		use futures_util::StreamExt;

		let result = report_twin_state_handle.report_twin_state(azure_iot_mqtt::ReportTwinStateRequest::Replace(
			vec![("start-time".to_string(), chrono::Utc::now().to_string().into())].into_iter().collect()
		)).await;
		let () = result.expect("couldn't report initial twin state");

		let mut interval = tokio::time::interval(report_twin_state_period);
		while interval.next().await.is_some() {
			let result = report_twin_state_handle.report_twin_state(azure_iot_mqtt::ReportTwinStateRequest::Patch(
				vec![("current-time".to_string(), chrono::Utc::now().to_string().into())].into_iter().collect()
			)).await;

			let () = result.expect("couldn't report twin state patch");
		}
	});
}


#[cfg(test)]
mod tests {
	const RAW_CONFIG_BASE64:&str = "ZXZlbnRzIHsgfQ0KDQoNCmh0dHAgew0KICAgIHByb3h5X2J1ZmZlcnMgMzIgMTYwazsgIA0KICAgIHByb3h5X2J1ZmZlcl9zaXplIDE2MGs7DQogICAgcHJveHlfcmVhZF90aW1lb3V0IDM2MDA7DQogICAgZXJyb3JfbG9nIC9kZXYvc3Rkb3V0IGluZm87DQogICAgYWNjZXNzX2xvZyAvZGV2L3N0ZG91dDsNCg0KICAgIHNlcnZlciB7DQogICAgICAgIGxpc3RlbiAke05HSU5YX0RFRkFVTFRfUE9SVH0gc3NsIGRlZmF1bHRfc2VydmVyOw0KDQogICAgICAgIGNodW5rZWRfdHJhbnNmZXJfZW5jb2Rpbmcgb247DQoNCiAgICAgICAgc3NsX2NlcnRpZmljYXRlICAgICAgICBzZXJ2ZXIuY3J0Ow0KICAgICAgICBzc2xfY2VydGlmaWNhdGVfa2V5ICAgIHNlcnZlci5rZXk7IA0KDQoNCiAgICAgICAgI2lmX3RhZyAke05HSU5YX0hBU19CTE9CX01PRFVMRX0NCiAgICAgICAgaWYgKCRodHRwX3hfbXNfYmxvYl90eXBlID0gQmxvY2tCbG9iKQ0KICAgICAgICB7DQogICAgICAgICAgICByZXdyaXRlIF4oLiopJCAvc3RvcmFnZSQxIGxhc3Q7DQogICAgICAgIH0gDQogICAgICAgICNlbmRpZl90YWcgJHtOR0lOWF9IQVNfQkxPQl9NT0RVTEV9DQoNCiAgICAgICAgI2lmX3RhZyAke05HSU5YX0hBU19SRUdJU1RSWV9NT0RVTEV9DQogICAgICAgIGxvY2F0aW9uIC92MiB7DQogICAgICAgICAgICBwcm94eV9odHRwX3ZlcnNpb24gMS4xOw0KICAgICAgICAgICAgcmVzb2x2ZXIgMTI3LjAuMC4xMTsNCiAgICAgICAgICAgIHNldCAkYmFja2VuZCAiaHR0cDovLyR7TkdJTkdfUkVHSVNUUllfTU9EVUxFX0FERFJFU1N9IjsNCiAgICAgICAgICAgIHByb3h5X3Bhc3MgICAgICAgICAgJGJhY2tlbmQ7DQogICAgICAgIH0NCiAgICAgICAjZW5kaWZfdGFnICR7TkdJTlhfSEFTX1JFR0lTVFJZX01PRFVMRX0NCg0KICAgICAgICAjaWZfdGFnICR7TkdJTlhfSEFTX0JMT0JfTU9EVUxFfQ0KICAgICAgICBsb2NhdGlvbiB+Xi9zdG9yYWdlLyguKil7DQogICAgICAgICAgICBwcm94eV9odHRwX3ZlcnNpb24gMS4xOw0KICAgICAgICAgICAgcmVzb2x2ZXIgMTI3LjAuMC4xMTsNCiAgICAgICAgICAgIHNldCAkYmFja2VuZCAiaHR0cDovLyR7TkdJTkdfQkxPQl9NT0RVTEVfTkFNRV9BRERSRVNTfSI7DQogICAgICAgICAgICBwcm94eV9wYXNzICAgICAgICAgICRiYWNrZW5kLyQxJGlzX2FyZ3MkYXJnczsNCiAgICAgICAgfQ0KICAgICAgICAjZW5kaWZfdGFnICR7TkdJTlhfSEFTX0JMT0JfTU9EVUxFfQ0KDQogICAgICAgICNpZl90YWcgJHtOR0lOWF9OT1RfUk9PVH0gICAgICANCiAgICAgICAgbG9jYXRpb24gL3sNCiAgICAgICAgICAgIHByb3h5X2h0dHBfdmVyc2lvbiAxLjE7DQogICAgICAgICAgICByZXNvbHZlciAxMjcuMC4wLjExOw0KICAgICAgICAgICAgc2V0ICRiYWNrZW5kICJodHRwczovLyR7R0FURVdBWV9IT1NUTkFNRX06NDQzIjsNCiAgICAgICAgICAgIHByb3h5X3Bhc3MgICAgICAgICAgJGJhY2tlbmQvJDEkaXNfYXJncyRhcmdzOw0KICAgICAgICB9DQogICAgICAgICNlbmRpZl90YWcgJHtOR0lOWF9OT1RfUk9PVH0NCiAgICB9DQp9";
    const RAW_CONFIG_TEXT:&str = "events { }\r\n\r\n\r\nhttp {\r\n    proxy_buffers 32 160k;  \r\n    proxy_buffer_size 160k;\r\n    proxy_read_timeout 3600;\r\n    error_log /dev/stdout info;\r\n    access_log /dev/stdout;\r\n\r\n    server {\r\n        listen ${NGINX_DEFAULT_PORT} ssl default_server;\r\n\r\n        chunked_transfer_encoding on;\r\n\r\n        ssl_certificate        server.crt;\r\n        ssl_certificate_key    server.key; \r\n\r\n\r\n        #if_tag ${NGINX_HAS_BLOB_MODULE}\r\n        if ($http_x_ms_blob_type = BlockBlob)\r\n        {\r\n            rewrite ^(.*)$ /storage$1 last;\r\n        } \r\n        #endif_tag ${NGINX_HAS_BLOB_MODULE}\r\n\r\n        #if_tag ${NGINX_HAS_REGISTRY_MODULE}\r\n        location /v2 {\r\n            proxy_http_version 1.1;\r\n            resolver 127.0.0.11;\r\n            set $backend \"http://${NGING_REGISTRY_MODULE_ADDRESS}\";\r\n            proxy_pass          $backend;\r\n        }\r\n       #endif_tag ${NGINX_HAS_REGISTRY_MODULE}\r\n\r\n        #if_tag ${NGINX_HAS_BLOB_MODULE}\r\n        location ~^/storage/(.*){\r\n            proxy_http_version 1.1;\r\n            resolver 127.0.0.11;\r\n            set $backend \"http://${NGING_BLOB_MODULE_NAME_ADDRESS}\";\r\n            proxy_pass          $backend/$1$is_args$args;\r\n        }\r\n        #endif_tag ${NGINX_HAS_BLOB_MODULE}\r\n\r\n        #if_tag ${NGINX_NOT_ROOT}      \r\n        location /{\r\n            proxy_http_version 1.1;\r\n            resolver 127.0.0.11;\r\n            set $backend \"https://${GATEWAY_HOSTNAME}:443\";\r\n            proxy_pass          $backend/$1$is_args$args;\r\n        }\r\n        #endif_tag ${NGINX_NOT_ROOT}\r\n    }\r\n}";
	const PARSED_CONFIG:&str = "events { }\r\n\r\n\r\nhttp {\r\n    proxy_buffers 32 160k;  \r\n    proxy_buffer_size 160k;\r\n    proxy_read_timeout 3600;\r\n    error_log /dev/stdout info;\r\n    access_log /dev/stdout;\r\n\r\n    server {\r\n        listen 443 ssl default_server;\r\n\r\n        chunked_transfer_encoding on;\r\n\r\n        ssl_certificate        server.crt;\r\n        ssl_certificate_key    server.key; \r\n\r\n\r\n        #if_tag 1\r\n        if ($http_x_ms_blob_type = BlockBlob)\r\n        {\r\n            rewrite ^(.*)$ /storage$1 last;\r\n        } \r\n        #endif_tag 1\r\n\r\n        \r\n\r\n        #if_tag 1\r\n        location ~^/storage/(.*){\r\n            proxy_http_version 1.1;\r\n            resolver 127.0.0.11;\r\n            set $backend \"http://module_blob:11002\";\r\n            proxy_pass          $backend/$1$is_args$args;\r\n        }\r\n        #endif_tag 1\r\n\r\n        \r\n    }\r\n}";
	use super::{*};

    #[test]
    fn env_var_tests() {
		//All environment variable tests are grouped in one test.
		//The reason is concurrency. Rust test are multi threaded by default
		//And environment variable are globals, so race condition happens.


		//Check config
		std::env::set_var("NGINX_DEFAULT_PORT", "443");
		std::env::set_var("NGINX_CERT_PATH", "server.crt");
		std::env::set_var("NGINX_PRIVATE_KEY_PATH", "server.key");
		std::env::set_var("NGINX_HAS_BLOB_MODULE", "1");
		std::env::set_var("NGING_BLOB_MODULE_NAME_ADDRESS", "module_blob:11002");	

		let byte_str = get_raw_config(RAW_CONFIG_BASE64).unwrap();
		let config = str::from_utf8(&byte_str).unwrap();
		assert_eq!(config, RAW_CONFIG_TEXT);  

		let config =  get_parsed_config(RAW_CONFIG_TEXT).unwrap();

		assert_eq!(&config, PARSED_CONFIG);

		//Check defaults variables set
		//unset all default variables
		for (key, _value) in PROXY_CONFIG_DEFAULT_VALUES.iter()
		{
			std::env::remove_var(key);	
		}
		set_default_env_vars();
		for (key, value) in PROXY_CONFIG_DEFAULT_VALUES.iter()
		{
			let var = std::env::var(key).unwrap();
			assert_eq!(*value, &var); 	
		}		

		//Check the the default function doesn't override user variable
		for (key, _value) in PROXY_CONFIG_DEFAULT_VALUES.iter()
		{
			std::env::set_var(key, "Dummy value");	
		}
		set_default_env_vars();
		for (key, value) in PROXY_CONFIG_DEFAULT_VALUES.iter()
		{
			let var = std::env::var(key).unwrap();
			assert_ne!(*value, &var); 	
		}		

	}
}
