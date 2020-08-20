// An example Edge module client.
//
// - Connects to Azure IoT Edge Hub using bare TLS or WebSockets.
// - Responds to direct method requests by returning the same payload.
// - Reports twin state once at start, then updates it periodically after.
//
//
// Example:
//
//     cargo run --example edge_module -- --use-websocket --will 'azure-iot-mqtt client unexpectedly disconnected'
// Some `use` statements have been omitted here for brevity
use anyhow::Context;
use tokio::process::Command;
use std::process::Stdio;
use tokio::sync::Notify;
use std::{pin::Pin, sync::Arc};
mod monitors;
use futures_util::future::{self};
use futures::Future;

#[tokio::main]
async fn main()  {
	env_logger::Builder::from_env(env_logger::Env::new().filter_or("AZURE_IOT_MQTT_LOG", "mqtt3=info,mqtt3::logging=info,azure_iot_mqtt=info,edge_module=info")).init();

	let runtime = tokio::runtime::Runtime::new().expect("couldn't initialize tokio runtime");

	let notify_need_reload_api_proxy = Arc::new(Notify::new());
	let notify_received_config = notify_need_reload_api_proxy.clone();
	let notify_certs_rotated = notify_need_reload_api_proxy.clone();

	let runtime_config_monitor = runtime.handle().clone();
	let config_task = 
		monitors::config_monitor::start(runtime_config_monitor, notify_received_config);

	let cert_task = monitors::certs_monitor::start(notify_certs_rotated);

	let loop_task = nginx_controller_loop(notify_need_reload_api_proxy);

	futures::future::join_all(vec![
		Box::pin(config_task) as Pin<Box<dyn Future<Output = ()>>>,
		Box::pin(cert_task) as Pin<Box<dyn Future<Output = ()>>>,
		Box::pin(loop_task) as Pin<Box<dyn Future<Output = ()>>>,
	]).await;
}

pub async fn nginx_controller_loop(notify_need_reload_api_proxy: Arc<Notify>){
	let program_path= "/usr/sbin/nginx";
	let args = vec!["-c".to_string(), "/app/nginx_config.conf".to_string(),"-g".to_string(),"daemon off;".to_string()];
	let name = "nginx";
	let stop_proxy_name = "stop nginx";
	let stop_proxy_program_path = "nginx";
	let stop_proxy_args = vec!["-s".to_string(), "stop".to_string()];


	//Wait for certificate rotation and for parse configuration.
	//This is just to avoid error at the beginning when nginx tries to start
	//but configuration is not ready
	notify_need_reload_api_proxy.notified().await;

	loop{
		//Make sure proxy is stopped by sending stop command. Otherwise port will be blocked
		let command = Command::new(stop_proxy_program_path).args(&stop_proxy_args)
		.spawn()
		.with_context(|| format!("Failed to start {:?} process.", stop_proxy_name)).expect("Cannot stop proxy!");
		command.await.expect("Error while trying to wait on stop proxy future");


		//Start nginx
		let child_nginx = Command::new(program_path).args(&args)
		.stdout(Stdio::inherit())
		.spawn()
		.with_context(|| format!("Failed to start {:?} process.", name)).expect("Cannot start proxy!");

		let signal_restart_nginx =  notify_need_reload_api_proxy.notified();
		futures::pin_mut!(child_nginx,signal_restart_nginx);
		
		//Wait for: either a signal to restart(cert rotation, new config) or the child to crash.
		future::select(child_nginx, signal_restart_nginx).await;
		log::info!("Restarting Proxy");
	}
}
//add pin utils
