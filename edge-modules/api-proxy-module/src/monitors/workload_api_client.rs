use chrono::Utc;
use anyhow::{Context, Result};

const PROXY_SERVER_CERT_VALIDITY_DAYS:i64 = 90; 

#[derive(Clone, Copy, Debug)]
enum Scheme {
	Http,
	Unix,
}

pub struct WorkloadAPIClient {
	module_id: String,
	generation_id: String,
	gateway_hostname: String,
	url_base: String,
	url_scheme: Scheme
}

#[derive(Debug, serde::Deserialize)]
struct TrustBundle{
	certificate: String
}

#[derive(Debug, serde::Deserialize)]
#[allow(non_snake_case)]
pub struct PrivateKey{
	bytes: String
}

#[derive(Debug, serde::Deserialize)]
#[allow(non_snake_case)]
pub struct ServerCerts{
	certificate: String,
	privateKey: PrivateKey,
	expiration: String
}


//Workload API: get Root certificate, server certificate and private key.
impl WorkloadAPIClient{

	pub fn new(module_id: String, generation_id:String, gateway_hostname:String, workload_url:String) -> Result<Self, anyhow::Error>{
		let workload_url: url::Url = workload_url.parse()?;
		
		let (url_scheme, url_base) = match workload_url.scheme() {
			"http" => (Scheme::Http, workload_url.to_string()),
			"unix" =>
				if cfg!(windows) {
					let mut workload_url = workload_url.clone();
					workload_url.set_scheme("file").expect(r#"changing the scheme of workload URI to "file" should not fail"#);
					let base = workload_url.to_file_path().ok().expect("path sources must have a valid path");
					let base = base.to_str().ok_or_else(|| anyhow::format_err!("Can't extract string"))?;
					(Scheme::Unix, base.to_owned())
				}
				else {
					(Scheme::Unix, workload_url.path().to_owned())
				},
			scheme => return Err(anyhow::anyhow!("Error {}", scheme.to_owned())),
		};

		Ok(WorkloadAPIClient{
			module_id,
			generation_id,
			gateway_hostname,
			url_base,
			url_scheme,
		})
	}

	pub fn  get_bundle_of_trust(&self)-> Result<String, anyhow::Error> {
		//Get Bundle of trust
		let url = match make_hyper_uri(self.url_scheme, &self.url_base, "/trust-bundle?api-version=2019-01-30") {
			Ok(url) => url,
			Err(_) => return Err(anyhow::anyhow!("Error")),
		};

		let resp: TrustBundle = reqwest::blocking::get(&url.to_string())?.json()?;

		Ok(resp.certificate)

	}

	pub fn get_server_cert_and_private_key(&self)-> Result<(String, String, String), anyhow::Error>{
		let args = format!("/modules/{}/genid/{}/certificate/server?api-version=2019-01-30", self.module_id, self.generation_id);
		let expiration = Utc::now().checked_add_signed(chrono::Duration::days(PROXY_SERVER_CERT_VALIDITY_DAYS))
		.context("Error could not generate expiration date for server certificate")?;
		let expiration_str = expiration.to_rfc3339();
		let body = format!("{{\"commonName\":\"{}\", \"expiration\":\"{}\"}}", self.gateway_hostname, expiration_str);
		let url = match make_hyper_uri(self.url_scheme, &self.url_base, &args) {
			Ok(url) => url,
			Err(_) => return Err(anyhow::anyhow!("Error")),
		};

		let client = reqwest::blocking::Client::new();
		let resp: ServerCerts = client.post(&url.to_string()).body(body).send()?.json()?;
		
		Ok((resp.certificate, resp.privateKey.bytes, resp.expiration))
	}
}


fn make_hyper_uri(scheme: Scheme, base: &str, path: &str) -> Result<hyper::Uri, Box<dyn std::error::Error + Send + Sync>> {
	match scheme {
		Scheme::Http => {
			let base = url::Url::parse(base)?;
			let url = base.join(path)?;
			let url = url.as_str().parse()?;
			Ok(url)
		},

		Scheme::Unix => Ok(hyper_uds::make_hyper_uri(base, path)?),
	}
}


#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    /*use http::StatusCode;
    use matches::assert_matches;*/
    use mockito::mock;
    use serde_json::json;
	use super::{*};

    #[test]
    fn test_makes_hyper_uri() {
		let scheme = Scheme::Unix;
		let base = "unix:///var/iotedge/workload.sock";
        let path = "/modules/$edgeHub/genid/12345678/certificate/server?api-version=2019-01-30";

        let uri = make_hyper_uri(scheme, base, &path).unwrap();
        assert!(uri.to_string().ends_with(path));
	}
	
	#[test]
	fn test_get_bundle_of_trust_server_cert_and_private_key(){
		let expiration = Utc::now() + Duration::days(90);
        let res = json!(
            {
                "privateKey": { "type": "key", "bytes": "PRIVATE KEY" },
                "certificate": "CERTIFICATE",
                "expiration": expiration.to_rfc3339()
            }
		);
		
        let _m = mock(
            "POST",
            "/modules/api_proxy/genid/0000/certificate/server?api-version=2019-01-30",
        )
        .with_status(201)
        .with_body(serde_json::to_string(&res).unwrap())
		.create();
		
		let module_id = String::from("api_proxy");
		let generation_id = String::from("0000");
		let gateway_hostname = String::from("dummy");
		let workload_url = mockito::server_url();

		let client = WorkloadAPIClient::new(module_id, generation_id, gateway_hostname, workload_url).expect("client");

		let (cert, private_key, expiry_time) = client.get_server_cert_and_private_key().expect("failed to get cert and private key");

        assert_eq!(cert, "CERTIFICATE");
        assert_eq!(private_key, "PRIVATE KEY");
        //assert_eq!(expiry_time.to_rfc3339(), expiration.to_rfc3339());
	}
/*
    #[tokio::test]
    async fn it_downloads_server_certificate() {
        let expiration = Utc::now() + Duration::days(90);
        let res = json!(
            {
                "privateKey": { "type": "key", "bytes": "PRIVATE KEY" },
                "certificate": "CERTIFICATE",
                "expiration": expiration.to_rfc3339()
            }
        );

        let _m = mock(
            "POST",
            "/modules/broker/genid/12345678/certificate/server?api-version=2019-01-30",
        )
        .with_status(201)
        .with_body(serde_json::to_string(&res).unwrap())
        .create();

        let client = workload(&mockito::server_url()).expect("client");
        let res = client
            .create_server_cert("broker", "12345678", "localhost", expiration)
            .await
            .unwrap();

        assert_eq!(res.private_key().bytes(), Some("PRIVATE KEY"));
        assert_eq!(res.certificate(), "CERTIFICATE");
        assert_eq!(res.expiration(), &expiration.to_rfc3339());
    }

    #[tokio::test]
    async fn it_handles_incorrect_status_for_create_server_cert() {
        let expiration = Utc::now() + Duration::days(90);
        let _m = mock(
            "POST",
            "/modules/broker/genid/12345678/certificate/server?api-version=2019-01-30",
        )
        .with_status(400)
        .with_body(r#"{"message":"Something went wrong"}"#)
        .create();

        let client = workload(&mockito::server_url()).expect("client");
        let res = client
            .create_server_cert("broker", "12345678", "locahost", expiration)
            .await
            .unwrap_err();

        assert_matches!(
            res,
            WorkloadError::Api(ApiError::UnsuccessfulResponse(StatusCode::BAD_REQUEST, _))
        )
    }

    #[tokio::test]
    async fn it_downloads_trust_bundle() {
        let res = json!( { "certificate": "CERTIFICATE" } );

        let _m = mock("GET", "/trust-bundle?api-version=2019-01-30")
            .with_status(200)
            .with_body(serde_json::to_string(&res).unwrap())
            .create();
        let client = workload(&mockito::server_url()).expect("client");
        let res = client.trust_bundle().await.unwrap();

        assert_eq!(res.certificate(), "CERTIFICATE");
    }*/
}