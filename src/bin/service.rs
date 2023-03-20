use wetc_buyback_backend::{
    response_from_items,
    response_from_hash,
    ParsedInput,
    Response,
    Client,
    Error,
};

use std::{
    net::SocketAddr,
    env::var,
};

use hyper::{self, service::service_fn, server::conn::Http, Body};
use futures::stream::TryStreamExt;
use tokio::net::TcpListener;
use firestore::FirestoreDb;
use serde_json::json;

static mut DB: Option<FirestoreDb> = None;
static mut CLIENT: Option<Client> = None;

#[tokio::main]
async fn main() {
    let listener: TcpListener = get_listener().await.unwrap();
    unsafe {
        CLIENT = Some(wetc_buyback_backend::get_client().await.unwrap());
        DB = Some(wetc_buyback_backend::get_db().await.unwrap());
    }

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|e| Error::ListenerAcceptError(e))
            .unwrap();

        tokio::task::spawn(async move {
            if let Err(err) = Http::new()
                .serve_connection(stream, service_fn(serve))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

macro_rules! unwrap_or_rep {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => return Ok(err_response(e)),
        }
    }
}

async fn serve(
    req: hyper::Request<Body>,
) -> Result<hyper::Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
    match req.method() {
        &hyper::Method::OPTIONS => return Ok(preflight_response()),
        _ => match validate(&req) {
            Some(invalid) => return Ok(invalid),
            None => (),
        },
    };

    let buf: Vec<u8> = unwrap_or_rep!(req
        .into_body()
        .try_fold(Vec::new(), |mut data, chunk| async move {
            data.extend_from_slice(&chunk);
            Ok(data)
        })
        .await
        .map_err(|e| Error::HyperRequestBodyError(e.into()))
    );

    let parsed_input = unwrap_or_rep!(ParsedInput::from_slice(&buf));
    let response: Response = match parsed_input {
        ParsedInput::Items((v, l)) => unwrap_or_rep!(response_from_items(
            v, l, get_db(), get_client(),
        )
            .await
        ),
        ParsedInput::Hash(h) => unwrap_or_rep!(response_from_hash(
            h, get_db(),
        )
            .await
        ),
    };

    match response.to_json() {
        Ok(j) => Ok(success_response(j)),
        Err(e) => Ok(err_response(e)),
    }
}

// Return a response only if the request is invalid
fn validate(_req: &hyper::Request<Body>) -> Option<hyper::Response<Body>> {
    None
}

fn get_client() -> &'static Client {
    unsafe {
        match &CLIENT {
            Some(c) => c,
            None => unreachable!(),
        }
    }
}

fn get_db() -> &'static FirestoreDb {
    unsafe {
        match &DB {
            Some(d) => d,
            None => unreachable!(),
        }
    }
}

fn preflight_response() -> hyper::Response<Body> {
    hyper::Response::builder()
        .status(hyper::StatusCode::OK)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Headers", "*")
        .header("Access-Control-Allow-Methods", "POST, OPTIONS")
        .body(Body::default())
        .unwrap()
}

fn success_response(json_body: String) -> hyper::Response<Body> {
    hyper::Response::builder()
        .status(hyper::StatusCode::OK)
        .header("Access-Control-Allow-Origin", "*")
        .header("Content-Type", "application/json")
        .body(Body::from(json_body))
        .unwrap()
}

fn err_response(error: Error) -> hyper::Response<Body> {
    hyper::Response::builder()
        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
        .header("Access-Control-Allow-Origin", "*")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({"error": format!("{}", error)}).to_string(),
        ))
        .unwrap()
}

async fn get_listener() -> Result<TcpListener, Error> {
    let socket: SocketAddr = var("BBBE_LISTENADDR")?
        .to_string()
        .parse()?;
    TcpListener::bind(socket)
        .await
        .map_err(|e| Error::ListenerBindError(e))
}
