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

async fn serve(
    req: hyper::Request<Body>,
) -> Result<hyper::Response<Body>, Box<dyn std::error::Error + Send + Sync>> {
    let buf: Vec<u8> = req
        .into_body()
        .try_fold(Vec::new(), |mut data, chunk| async move {
            data.extend_from_slice(&chunk);
            Ok(data)
        })
        .await?;

    let parsed_input = match ParsedInput::from_slice(&buf) {
        Ok(pi) => pi,
        Err(e) => return Ok(err_response(e)),
    };
    let response: Response = match parsed_input {
        ParsedInput::Items((v, l)) => match response_from_items(
            v,
            l,
            get_db(),
            get_client(),
        )
            .await
        {
            Ok(rep) => rep,
            Err(e) => return Ok(err_response(e)),
        },
        ParsedInput::Hash(h) => match response_from_hash(
            h,
            get_db(),
        )
            .await
        {
            Ok(rep) => rep,
            Err(e) => return Ok(err_response(e)),
        },
    };

    match response.to_json() {
        Ok(j) => Ok(hyper::Response::new(Body::from(j))),
        Err(e) => Ok(err_response(e)),
    }
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

fn err_response(error: Error) -> hyper::Response<Body> {
    hyper::Response::builder()
        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(format!("{:?}", error)))
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