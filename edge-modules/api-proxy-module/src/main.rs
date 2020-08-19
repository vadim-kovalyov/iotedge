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
use std::sync::Arc;
mod monitors;
use futures_util::future::{self};

fn main()  {
	env_logger::Builder::from_env(env_logger::Env::new().filter_or("AZURE_IOT_MQTT_LOG", "mqtt3=info,mqtt3::logging=info,azure_iot_mqtt=info,edge_module=info")).init();

	
	let runtime = tokio::runtime::Runtime::new().expect("couldn't initialize tokio runtime");

	let notify_need_reload_api_proxy = Arc::new(Notify::new());
	let notify_received_config = notify_need_reload_api_proxy.clone();
	let notify_certs_rotated = notify_need_reload_api_proxy.clone();

	let runtime_config_monitor = runtime.handle().clone();
	runtime.handle().spawn_blocking(move || monitors::config_monitor::start(runtime_config_monitor, notify_received_config));

	let runtime_cert_monitor = runtime.handle().clone();
	runtime.handle().spawn_blocking(move ||monitors::certs_monitor::start(runtime_cert_monitor, notify_certs_rotated));

	let runtime_watchdog = runtime.handle().clone();
	runtime.handle().spawn_blocking(move ||watch_dog_loop(runtime_watchdog, notify_need_reload_api_proxy));

	//@Todo find a cleaner way to wait.
	loop{
		std::thread::sleep(std::time::Duration::new(1000, 0));
	}
}
/*

fn watch_dog_loop(runtime_handle: tokio::runtime::Handle, notify_need_reload_api_proxy: Arc<Notify>){
	loop {
		let name = "reload nginx";
		let reload_proxy_program_path = "nginx";
		let reload_proxy_args = vec!["-s".to_string(), "reload".to_string()];
	
		//Block until we get a signal to reload nginx
		runtime_handle.block_on(notify_need_reload_api_proxy.notified());
		let child = Command::new(reload_proxy_program_path).args(&reload_proxy_args)
		.stdout(Stdio::inherit())
		.spawn()
		.with_context(|| format!("Failed to start {:?} process.", name)).expect("Cannot reload proxy!");

		runtime_handle.block_on(child).expect("Error while trying to wait on reload proxy future");
	}
}*/


pub fn watch_dog_loop(runtime_handle: tokio::runtime::Handle, notify_need_reload_api_proxy: Arc<Notify>){
	let program_path= "/usr/sbin/nginx";
	let args = vec!["-c".to_string(), "/app/nginx_config.conf".to_string(),"-g".to_string(),"daemon off;".to_string()];
	let name = "nginx";
	let stop_proxy_name = "stop nginx";
	let stop_proxy_program_path = "nginx";
	let stop_proxy_args = vec!["-s".to_string(), "stop".to_string()];


	//Wait for certificate rotation and for parse configuration.
	//This is just to avoid error at the beginning when nginx tries to start
	//but configuration is not ready
	runtime_handle.block_on(notify_need_reload_api_proxy.notified());
	runtime_handle.block_on(notify_need_reload_api_proxy.notified());

	loop{
		//Make sure proxy is stopped by sending stop command. Otherwise port will be blocked
		let command = Command::new(stop_proxy_program_path).args(&stop_proxy_args)
		.spawn()
		.with_context(|| format!("Failed to start {:?} process.", stop_proxy_name)).expect("Cannot stop proxy!");
		runtime_handle.block_on(command).expect("Error while trying to wait on stop proxy future");


		//Start nginx
		let child_nginx = Command::new(program_path).args(&args)
		.stdout(Stdio::inherit())
		.spawn()
		.with_context(|| format!("Failed to start {:?} process.", name)).expect("Cannot start proxy!");

		let signal_restart_nginx =  notify_need_reload_api_proxy.notified();
		futures::pin_mut!(child_nginx,signal_restart_nginx);
		
		//Wait for: either a signal to restart(cert rotation, new config) or the child to crash.
		runtime_handle.block_on(future::select(child_nginx, signal_restart_nginx));
		log::info!("Restarting Proxy");
	}
}
//add pin utils
